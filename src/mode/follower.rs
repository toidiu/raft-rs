use crate::{
    io::{ServerIO, IO_BUF_LEN},
    log::{MatchOutcome, TermIdx},
    mode::Context,
    rpc::{AppendEntries, RequestVote, Rpc},
};
use s2n_codec::{EncoderBuffer, EncoderValue};
use std::cmp::{min, Ordering};

#[derive(Debug, Default)]
pub struct Follower;

impl Follower {
    pub fn on_follower<IO: ServerIO>(&mut self, _context: &mut Context<IO>) {}

    pub fn on_recv<IO: ServerIO>(&mut self, rpc: crate::rpc::Rpc, context: &mut Context<IO>) {
        //% Compliance:
        //% Respond to RPCs from candidates and leaders
        match rpc {
            Rpc::RV(request_vote) => self.on_recv_request_vote(request_vote, context),
            Rpc::AE(append_entries) => self.on_recv_append_entries(append_entries, context),
            Rpc::RVR(_) | Rpc::AER(_) => (),
        }
    }

    fn on_recv_request_vote<IO: ServerIO>(
        &mut self,
        request_vote: RequestVote,
        context: &mut Context<IO>,
    ) {
        let current_term = context.state.current_term;

        //% Compliance:
        //% Reply false if term < currentTerm (§5.1)
        let term_criteria = {
            let rpc_term_lt_current_term = request_vote.term < current_term;
            !rpc_term_lt_current_term
        };

        //% Compliance:
        //% If candidate’s log is at least as up-to-date as receiver’s log
        let log_up_to_date_criteria = self.log_up_to_date(&request_vote, context);

        let voted_for_criteria = if let Some(voted_for) = context.state.voted_for {
            //% Compliance:
            //% and votedFor is candidateId, grant vote (§5.2, §5.4)
            voted_for == request_vote.candidate_id
        } else {
            //% Compliance:
            //% and votedFor is null, grant vote (§5.2, §5.4)
            true
        };

        let grant_vote = term_criteria && log_up_to_date_criteria && voted_for_criteria;
        if grant_vote {
            // set local state to capture granting the vote
            context.state.voted_for = Some(request_vote.candidate_id);
        }

        let candidate_io = &mut context
            .peer_map
            .get_mut(&request_vote.candidate_id)
            .unwrap()
            .io;
        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_request_vote_resp(current_term, grant_vote);
        candidate_io.send(buf.as_mut_slice().to_vec());
    }

    fn log_up_to_date<IO: ServerIO>(
        &mut self,
        request_vote: &RequestVote,
        context: &mut Context<IO>,
    ) -> bool {
        //% Compliance:
        //% `up-to-date`: a log is considered more up-to-date than another log if:
        //%	- compare the index and term of the last entry of A's and B's log
        let rpc_term_idx = request_vote.last_log_term_idx;
        let log_term_idx = context.state.log.last_term_idx();
        let term_cmp = rpc_term_idx.term.cmp(&log_term_idx.term);
        let idx_cmp = rpc_term_idx.idx.cmp(&log_term_idx.idx);
        match (term_cmp, idx_cmp) {
            //% Compliance:
            //%	- if the entries have different term: the higher term is more up-to-date
            (Ordering::Greater, _) => true,
            (Ordering::Less, _) => false,
            //% Compliance:
            //%	- if the term is the same: the longer log (higher index) is more up-to-date
            (Ordering::Equal, Ordering::Less) => false,
            (Ordering::Equal, Ordering::Equal) => true,
            (Ordering::Equal, Ordering::Greater) => true,
        }
    }

    fn on_recv_append_entries<IO: ServerIO>(
        &mut self,
        append_entries: AppendEntries,
        context: &mut Context<IO>,
    ) {
        let current_term = context.state.current_term;

        //% Compliance:
        //% Reply false if term < currentTerm (§5.1)
        let rpc_term_lt_current_term = append_entries.term < current_term;
        //% Compliance:
        //% Reply false if log doesn’t contain an entry at prevLogIndex whose term
        //% matches prevLogTerm (§5.3)
        let log_contains_matching_prev_entry = matches!(
            context
                .state
                .log
                .entry_matches(append_entries.prev_log_term_idx),
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
            //% If an existing entry conflicts with a new one (same index but different terms),
            //% delete the existing entry and all that follow it (§5.3)
            //
            //% Compliance:
            //% Append any new entries not already in the log
            let mut entry_idx = append_entries.prev_log_term_idx.idx + 1;
            for entry in append_entries.entries.into_iter() {
                let _match_outcome = context.state.log.match_leaders_log(entry, entry_idx);
                entry_idx += 1;
            }

            //% Compliance:
            //% If leaderCommit > commitIndex, set commitIndex = min(leaderCommit, index of
            //% last new entry)
            assert!(
                append_entries.leader_commit_idx <= context.state.log.prev_idx(),
                "leader_commit_idx should not be greater than the number of enties in the log"
            );
            if append_entries.leader_commit_idx > context.state.commit_idx {
                context.state.commit_idx = min(
                    append_entries.leader_commit_idx,
                    context.state.log.prev_idx(),
                );
            }
        }

        let leader_io = &mut context
            .peer_map
            .get_mut(&append_entries.leader_id)
            .unwrap()
            .io;
        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_append_entry_resp(current_term, response).encode_mut(&mut buf);
        leader_io.send(buf.as_mut_slice().to_vec());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        io::testing::helper_inspect_sent_rpc,
        log::{Entry, Idx, Term, TermIdx},
        peer::Peer,
        server::ServerId,
        state::State,
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;

    #[tokio::test]
    async fn test_recv_append_entries() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let leader_id_fill = 2;
        let leader_id = ServerId::new([leader_id_fill; 16]);
        let mut peer_map = Peer::mock_as_map(&[leader_id_fill]);
        let mut state = State::new(timeout, &peer_map);
        let current_term = Term::from(2);
        state.current_term = current_term;

        let mut follower = Follower;
        let leader_commit_idx = Idx::from(0);
        let prev_log_term_idx = TermIdx::initial();

        // Expect response true
        // - but no entries are sent
        {
            let mut context = Context {
                server_id,
                state: &mut state,
                peer_map: &mut peer_map,
            };
            // constust RPC to recv
            let recv_rpc = Rpc::new_append_entry(
                current_term,
                leader_id,
                prev_log_term_idx,
                leader_commit_idx,
                vec![],
            );
            follower.on_recv(recv_rpc, &mut context);

            let leader_io = &mut peer_map.get_mut(&leader_id).unwrap().io;
            let rpc = helper_inspect_sent_rpc(leader_io);
            let expected_rpc = Rpc::new_append_entry_resp(current_term, true);
            assert_eq!(expected_rpc, rpc);
            assert!(state.log.entries.is_empty());
        }

        // Expect response false
        // - term < current_term
        {
            let mut context = Context {
                server_id,
                state: &mut state,
                peer_map: &mut peer_map,
            };
            let prev_log_term_idx = TermIdx::initial();
            let recv_rpc = Rpc::new_append_entry(
                current_term - 1,
                leader_id,
                prev_log_term_idx,
                leader_commit_idx,
                vec![Entry::new(current_term, 3), Entry::new(current_term, 6)],
            );
            // on_recv AppendEntries
            follower.on_recv(recv_rpc, &mut context);

            let leader_io = &mut peer_map.get_mut(&leader_id).unwrap().io;
            let rpc = helper_inspect_sent_rpc(leader_io);
            let expected_rpc = Rpc::new_append_entry_resp(current_term, false);
            assert_eq!(expected_rpc, rpc);
            assert!(state.log.entries.is_empty());
        }

        // Expect response false
        // - log doesnt contain prev entry
        {
            let mut context = Context {
                server_id,
                state: &mut state,
                peer_map: &mut peer_map,
            };
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
            follower.on_recv(recv_rpc, &mut context);

            let leader_io = &mut peer_map.get_mut(&leader_id).unwrap().io;
            let rpc = helper_inspect_sent_rpc(leader_io);
            let expected_rpc = Rpc::new_append_entry_resp(current_term, false);
            assert_eq!(expected_rpc, rpc);
            assert!(state.log.entries.is_empty());
        }

        // Expect response true
        //  - process entries
        //  - update commit_idx
        let leader_commit_idx = Idx::from(1);
        {
            assert!(state.log.entries.is_empty());
            assert_eq!(state.commit_idx, Idx::initial());

            let mut context = Context {
                server_id,
                state: &mut state,
                peer_map: &mut peer_map,
            };
            // construct RPC to recv
            let recv_rpc = Rpc::new_append_entry(
                current_term,
                leader_id,
                prev_log_term_idx,
                leader_commit_idx,
                vec![Entry::new(current_term, 3), Entry::new(current_term, 6)],
            );
            follower.on_recv(recv_rpc, &mut context);

            let leader_io = &mut peer_map.get_mut(&leader_id).unwrap().io;
            let rpc = helper_inspect_sent_rpc(leader_io);
            let expected_rpc = Rpc::new_append_entry_resp(current_term, true);
            assert_eq!(expected_rpc, rpc);

            // expect received entries to be in the log
            assert!(state.log.entries.len() == 2);
            assert_eq!(state.log.entries[0], Entry::new(current_term, 3));
            assert_eq!(state.log.entries[1], Entry::new(current_term, 6));

            // commit_idx should be updated
            assert_eq!(state.commit_idx, leader_commit_idx);
        }
    }
}
