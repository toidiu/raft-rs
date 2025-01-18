use crate::{
    io::{ServerIO, IO_BUF_LEN},
    log::MatchOutcome,
    mode::Context,
    rpc::{AppendEntries, RequestVote, Rpc},
};
use s2n_codec::{EncoderBuffer, EncoderValue};
use std::cmp::min;

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
        let log_up_to_date_criteria = context
            .state
            .log
            .is_candidate_log_up_to_date(&request_vote.last_log_term_idx);

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
        Rpc::new_request_vote_resp(current_term, grant_vote).encode_mut(&mut buf);
        candidate_io.send(buf.as_mut_slice().to_vec());
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
        let leader_commit_idx = Idx::initial();
        let prev_log_term_idx = TermIdx::initial();

        // Expect response true
        // - but no entries are sent
        {
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

    #[tokio::test]
    async fn test_recv_request_vote_empty_log() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let candidate_id_fill = 2;
        let candidate_id = ServerId::new([candidate_id_fill; 16]);
        let mut peer_map = Peer::mock_as_map(&[candidate_id_fill]);
        let mut state = State::new(timeout, &peer_map);
        let current_term = Term::initial();
        state.current_term = current_term;
        assert!(state.log.entries.is_empty());

        let mut follower = Follower;
        let rpc_term_idx_initial = TermIdx::initial();

        // Expect grant_vote: empty Log, TermIdx initial
        {
            let mut context = Context {
                server_id,
                state: &mut state,
                peer_map: &mut peer_map,
            };
            // construct RPC to recv
            let recv_rpc = Rpc::new_request_vote(current_term, candidate_id, rpc_term_idx_initial);
            follower.on_recv(recv_rpc, &mut context);

            let candidate_io = &mut peer_map.get_mut(&candidate_id).unwrap().io;
            let rpc = helper_inspect_sent_rpc(candidate_io);
            let expected_rpc = Rpc::new_request_vote_resp(current_term, true);
            assert_eq!(expected_rpc, rpc);
        }

        // Expect grant_vote: empty Log, TermIdx > initial
        {
            let mut context = Context {
                server_id,
                state: &mut state,
                peer_map: &mut peer_map,
            };
            // construct RPC to recv
            let rpc_log_term_idx = TermIdx::builder()
                .with_term(Term::from(1))
                .with_idx(Idx::from(1));
            let rpc_term = Term::from(1);
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_log_term_idx);
            follower.on_recv(recv_rpc, &mut context);

            let candidate_io = &mut peer_map.get_mut(&candidate_id).unwrap().io;
            let rpc = helper_inspect_sent_rpc(candidate_io);
            let expected_rpc = Rpc::new_request_vote_resp(current_term, true);
            assert_eq!(expected_rpc, rpc);
        }
    }

    #[tokio::test]
    async fn test_recv_request_vote() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let candidate_id_fill = 2;
        let candidate_id = ServerId::new([candidate_id_fill; 16]);
        let mut peer_map = Peer::mock_as_map(&[candidate_id_fill]);
        let mut state = State::new(timeout, &peer_map);
        let term_1 = Term::from(1);
        let term_2 = Term::from(2);
        state.current_term = term_2;

        state
            .log
            .push(vec![Entry::new(term_1, 3), Entry::new(term_2, 6)]);

        let mut follower = Follower;

        // TODO
        // == Equal TermIdx ==
        // == Different Term ==
        // term <
        // term >
        // == Same Term ==
        // idx <
        // idx >

        // Expect NOT to grant_vote:
        // local Log: [ [term: 1] [term: 2] ]
        {
            let mut context = Context {
                server_id,
                state: &mut state,
                peer_map: &mut peer_map,
            };
            // construct RPC to recv
            let rpc_log_term_idx = TermIdx::builder()
                .with_term(Term::from(1))
                .with_idx(Idx::from(1));
            let rpc_term = Term::from(1);
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_log_term_idx);
            follower.on_recv(recv_rpc, &mut context);

            let candidate_io = &mut peer_map.get_mut(&candidate_id).unwrap().io;
            let rpc = helper_inspect_sent_rpc(candidate_io);
            let expected_rpc = Rpc::new_request_vote_resp(term_2, false);
            assert_eq!(expected_rpc, rpc);
        }
    }
}
