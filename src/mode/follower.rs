use crate::{
    io::{ServerIO, IO_BUF_LEN},
    log::MatchOutcome,
    mode::Context,
    rpc::{AppendEntries, Rpc},
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
            Rpc::RV(_request_vote_state) => todo!(),
            Rpc::AE(append_entries_state) => {
                let AppendEntries {
                    term,
                    leader_id,
                    prev_log_term_idx,
                    leader_commit_idx,
                    entries,
                } = append_entries_state;
                let leader_io = &mut context.peer_map.get_mut(&leader_id).unwrap().io;

                //% Compliance:
                //% Reply false if term < currentTerm (§5.1)
                let term_lt_current_term = term < context.state.current_term;
                //% Compliance:
                //% Reply false if log doesn’t contain an entry at prevLogIndex whose term
                //% matches prevLogTerm (§5.3)
                let log_contains_matching_prev_entry = matches!(
                    context.state.log.entry_matches(prev_log_term_idx),
                    MatchOutcome::Match
                );
                #[allow(clippy::needless_bool)]
                let response = if term_lt_current_term || !log_contains_matching_prev_entry {
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
                    let mut entry_idx = prev_log_term_idx.idx + 1;
                    for entry in entries.into_iter() {
                        let _match_outcome = context.state.log.match_leaders_log(entry, entry_idx);
                        entry_idx += 1;
                    }

                    //% Compliance:
                    //% If leaderCommit > commitIndex, set commitIndex = min(leaderCommit, index of
                    //% last new entry)
                    assert!(
                        leader_commit_idx <= context.state.log.prev_idx(),
                        "leader_commit_idx should not be greater than the number of enties in the log"
                    );
                    if leader_commit_idx > context.state.commit_idx {
                        context.state.commit_idx =
                            min(leader_commit_idx, context.state.log.prev_idx());
                    }
                }

                let mut slice = vec![0; IO_BUF_LEN];
                let mut buf = EncoderBuffer::new(&mut slice);
                let term = context.state.current_term;
                Rpc::new_append_entry_resp(term, response).encode_mut(&mut buf);
                leader_io.send(buf.as_mut_slice().to_vec());
            }
            Rpc::RVR(_) | Rpc::AER(_) => (),
        }
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
