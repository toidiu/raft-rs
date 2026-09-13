use crate::{
    mode::ModeTransition,
    packet::{append_entries::EntriesLenTypeEncoding, AppendEntries, Rpc},
    queue::ServerEgress,
    server::PeerId,
    state::{log::MatchOutcome, raft_state::RaftState, state_machine::CurrentMode},
};
use std::cmp::min;

#[derive(Debug, Default)]
pub struct Follower {
    //% Compliance:
    //% leaderId: so follower can redirect clients
    //
    // The Leader this Follower last accepted an AppendEntries from.
    current_leader: Option<PeerId>,
}

impl Follower {
    pub fn on_follower(&mut self) {}

    /// The Leader to redirect a client to, if this Follower knows of one.
    pub fn current_leader(&self) -> Option<PeerId> {
        self.current_leader
    }

    pub fn on_timeout(&mut self) -> ModeTransition {
        //% Compliance:
        //% If election timeout elapses without receiving AppendEntries RPC from current
        //% leader or granting vote to candidate: convert to candidate
        //
        //% Compliance:
        //% a follower that receives no communication (election timeout) assumes there is no viable
        //% leader
        //%	- increments its current term
        //%	- transitions to `candidate`
        //%	- votes for itself
        //%	- issues a RequestVote in parallel to other servers
        //
        // A new election is started once the server transitions to Candidate
        ModeTransition::ToCandidate
    }

    pub fn on_recv<E: ServerEgress>(
        &mut self,
        peer_id: PeerId,
        rpc: &Rpc,
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        //% Compliance:
        //% Respond to RPCs from candidates and leaders
        match rpc {
            Rpc::RequestVote(request_vote) => request_vote.on_recv(peer_id, raft_state, io_egress),
            Rpc::AppendEntry(append_entries) => {
                self.on_recv_append_entries(peer_id, append_entries, raft_state, io_egress)
            }
            Rpc::RequestVoteResp(_) | Rpc::AppendEntryResp(_) => {
                // Ignore since a Follower doesn't send AppendEntry or RequestVote
                debug_assert!(false);
            }
        }
    }

    fn on_recv_append_entries<E: ServerEgress>(
        &mut self,
        peer_id: PeerId,
        append_entries: &AppendEntries,
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        let AppendEntries {
            term,
            leader_id: _,
            prev_log_term_idx,
            leader_commit_idx,
            entries,
        } = append_entries;

        let current_term = raft_state.current_term;

        //% Compliance:
        //% Reply false if term < currentTerm (§5.1)
        let rpc_term_lt_current_term = term < &current_term;
        //% Compliance:
        //% Reply false if log doesn’t contain an entry at prevLogIndex whose term
        //% matches prevLogTerm (§5.3)
        let log_contains_matching_prev_entry = matches!(
            raft_state.log.entry_matches(*prev_log_term_idx),
            MatchOutcome::Match
        );
        #[allow(clippy::needless_bool)]
        let response = if rpc_term_lt_current_term || !log_contains_matching_prev_entry {
            false
        } else {
            true
        };

        if response {
            //% Compliance:
            //% leaderId: so follower can redirect clients
            //
            // Accepting the RPC means recognizing the sender as the current Leader.
            self.current_leader = Some(peer_id);

            //% Compliance:
            //% If election timeout elapses without receiving AppendEntries RPC from current
            //% leader or granting vote to candidate: convert to candidate
            //
            // A heartbeat landed, so the Leader is alive. Without this a Follower campaigns against
            // a healthy Leader as soon as its own timeout fires.
            raft_state.timeout.reset_timeout();

            //% Compliance:
            //% If an existing entry conflicts with a new one (same index but different terms),
            //% delete the existing entry and all that follow it (§5.3)
            //
            //% Compliance:
            //% Append any new entries not already in the log
            // The RPC's entries are contiguous and start immediately after prev.
            for (offset, entry) in entries.iter().enumerate() {
                // Log idx is 1-indexed.
                let entry_idx = prev_log_term_idx.idx + 1 + offset as u64;
                let _match_outcome = raft_state
                    .log
                    .update_to_match_leaders_log(entry.clone(), entry_idx);
            }

            //% Compliance:
            //% If leaderCommit > commitIndex, set commitIndex = min(leaderCommit, index of
            //% last new entry)
            if leader_commit_idx > raft_state.commit_idx() {
                let commit_idx = min(*leader_commit_idx, raft_state.log.last_idx());
                raft_state.update_commit_idx(commit_idx, &[peer_id], CurrentMode::Follower);
            }
        }

        // The entries are stored all or nothing, so a success accounts for every entry sent.
        let entries_cnt = if response {
            entries.len() as EntriesLenTypeEncoding
        } else {
            0
        };

        let leader_io = io_egress;
        let rpc =
            Rpc::new_append_entry_resp(current_term, response, *prev_log_term_idx, entries_cnt);
        leader_io.send_packet(peer_id, rpc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        queue::testing::{helper_inspect_one_sent_packet, MockIo},
        server::ServerId,
        state::{
            entry::Entry,
            log::{Idx, Term, TermIdx},
            raft_state::RaftState,
        },
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;

    #[tokio::test]
    async fn test_recv_append_entries() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let leader_id = ServerId::new([2; 16]);
        let peer_id = PeerId::new([10; 16]);
        let mut state = RaftState::new(timeout);
        let current_term = Term::from(2);
        state.current_term = current_term;

        let mut follower = Follower::default();
        let leader_commit_idx = Idx::initial();
        let prev_log_term_idx = TermIdx::initial();

        let mut io = MockIo::new(leader_id);

        // Expect response true
        // - but no entries are sent
        {
            // construct RPC to recv
            let recv_rpc = Rpc::new_append_entry(
                current_term,
                leader_id,
                prev_log_term_idx,
                leader_commit_idx,
                vec![],
            );
            follower.on_recv(peer_id, &recv_rpc, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc =
                Rpc::new_append_entry_resp(current_term, true, TermIdx::initial(), 0);
            assert_eq!(&expected_rpc, packet.rpc());
            assert!(state.log.test_is_empty());
        }

        // Expect response false
        // - term < current_term
        {
            let prev_log_term_idx = TermIdx::initial();
            let recv_rpc = Rpc::new_append_entry(
                current_term - 1,
                leader_id,
                prev_log_term_idx,
                leader_commit_idx,
                vec![Entry::new(current_term, 3), Entry::new(current_term, 6)],
            );
            // on_recv AppendEntries
            follower.on_recv(peer_id, &recv_rpc, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc =
                Rpc::new_append_entry_resp(current_term, false, TermIdx::initial(), 0);
            assert_eq!(&expected_rpc, packet.rpc());
            assert!(state.log.test_is_empty());
        }

        // Expect response false
        // - log doesnt contain prev entry
        {
            let prev_log_term_idx = TermIdx::builder()
                .with_term(Term::from(1))
                .with_idx(Idx::from(1));
            let recv_rpc = Rpc::new_append_entry(
                current_term,
                leader_id,
                prev_log_term_idx,
                leader_commit_idx,
                vec![Entry::new(current_term, 3), Entry::new(current_term, 6)],
            );
            // on_recv AppendEntries
            follower.on_recv(peer_id, &recv_rpc, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_append_entry_resp(
                current_term,
                false,
                TermIdx::builder()
                    .with_term(Term::from(1))
                    .with_idx(Idx::from(1)),
                0,
            );
            assert_eq!(&expected_rpc, packet.rpc());
            assert!(state.log.test_is_empty());
        }

        // Expect response true
        //  - process entries
        //  - update commit_idx
        let leader_commit_idx = Idx::from(1);
        {
            assert!(state.log.test_is_empty());
            assert_eq!(state.commit_idx(), &Idx::initial());

            // construct RPC to recv
            let recv_rpc = Rpc::new_append_entry(
                current_term,
                leader_id,
                prev_log_term_idx,
                leader_commit_idx,
                vec![Entry::new(current_term, 3), Entry::new(current_term, 6)],
            );
            follower.on_recv(peer_id, &recv_rpc, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc =
                Rpc::new_append_entry_resp(current_term, true, TermIdx::initial(), 2);
            assert_eq!(&expected_rpc, packet.rpc());

            // expect received entries to be in the log
            assert!(state.log.test_len() == 2);
            assert_eq!(state.log.test_get_unchecked(1), Entry::new(current_term, 3));
            assert_eq!(state.log.test_get_unchecked(2), Entry::new(current_term, 6));

            // commit_idx should be updated
            assert_eq!(state.commit_idx(), &leader_commit_idx);
        }
    }

    // A prev_log_term_idx of TermIdx::initial is used to repair the Follower logs.
    #[tokio::test]
    async fn test_recv_append_entries_initial_prev_with_non_empty_log() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let leader_id = ServerId::new([2; 16]);
        let peer_id = PeerId::new([10; 16]);
        let mut state = RaftState::new(timeout);
        let old_term = Term::from(1);
        let current_term = Term::from(2);
        state.current_term = current_term;

        // The Follower holds two entries from an earlier term, which the Leader will overwrite.
        state
            .log
            .test_append_entries(vec![Entry::new(old_term, 3), Entry::new(old_term, 6)]);

        let mut follower = Follower::default();
        let mut io = MockIo::new(leader_id);

        let recv_rpc = Rpc::new_append_entry(
            current_term,
            leader_id,
            TermIdx::initial(),
            Idx::initial(),
            vec![Entry::new(current_term, 9)],
        );
        follower.on_recv(peer_id, &recv_rpc, &mut state, &mut io);

        let packet = helper_inspect_one_sent_packet(&mut io);
        let expected_rpc = Rpc::new_append_entry_resp(current_term, true, TermIdx::initial(), 1);
        assert_eq!(&expected_rpc, packet.rpc());

        // The conflicting entry at idx 1 is replaced and everything following it is dropped.
        assert_eq!(state.log.test_len(), 1);
        assert_eq!(state.log.test_get_unchecked(1), Entry::new(current_term, 9));
    }

    // A Leader's commit_idx can outrun a lagging Follower's log, so the Follower clamps it to its
    // own last_idx instead of trusting it.
    #[tokio::test]
    async fn test_recv_append_entries_leader_commit_idx_past_end_of_log() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let leader_id = ServerId::new([2; 16]);
        let peer_id = PeerId::new([10; 16]);
        let mut state = RaftState::new(timeout);
        let current_term = Term::from(2);
        state.current_term = current_term;

        let mut follower = Follower::default();
        let mut io = MockIo::new(leader_id);

        // The Leader has committed through idx 5 but only sends the first entry, so the Follower
        // ends this RPC with a single entry in its log.
        let leader_commit_idx = Idx::from(5);
        let recv_rpc = Rpc::new_append_entry(
            current_term,
            leader_id,
            TermIdx::initial(),
            leader_commit_idx,
            vec![Entry::new(current_term, 3)],
        );
        follower.on_recv(peer_id, &recv_rpc, &mut state, &mut io);

        let packet = helper_inspect_one_sent_packet(&mut io);
        let expected_rpc = Rpc::new_append_entry_resp(current_term, true, TermIdx::initial(), 1);
        assert_eq!(&expected_rpc, packet.rpc());

        // commit_idx is clamped to the last entry the Follower actually holds. Committing idx 5
        // would mark entries it has never seen as committed.
        assert_eq!(state.log.test_len(), 1);
        assert_eq!(state.commit_idx(), &Idx::from(1));
    }
}
