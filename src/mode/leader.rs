use crate::{
    log::{Idx, TermIdx},
    mode::Mode,
    packet::{AppendEntriesResp, Rpc},
    queue::ServerEgress,
    raft_state::RaftState,
    server::{PeerId, ServerId},
    state_machine::CurrentMode,
};
use std::{collections::BTreeMap, ops::Sub};

#[derive(Debug, Default)]
pub struct Leader {
    // ==== Volatile state on leaders ====
    //% Compliance:
    //% `nextIndex[]` for each server, index of the next log entry to send to that server
    //% (initialized to leader last log index + 1)
    pub next_idx: BTreeMap<PeerId, Idx>,

    //% Compliance:
    //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server
    //% (initialized to 0, increases monotonically)
    pub match_idx: BTreeMap<PeerId, Idx>,
}

impl Leader {
    pub fn new(peer_list: &[PeerId], raft_state: &mut RaftState) -> Self {
        let mut next_idx = BTreeMap::new();
        let mut match_idx = BTreeMap::new();

        //% Compliance:
        //% `nextIndex[]` for each server, index of the next log entry to send to that server
        //% (initialized to leader last log index + 1)
        let next_log_idx = raft_state.log.last_idx() + 1;

        //% Compliance:
        //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server
        //% (initialized to 0, increases monotonically)
        let initial_idx = Idx::initial();

        for peer in peer_list.iter() {
            next_idx.insert(*peer, next_log_idx);
            match_idx.insert(*peer, initial_idx);
        }

        Leader {
            next_idx,
            match_idx,
        }
    }

    pub fn on_leader<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerId],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        //% Compliance:
        //% "Reinitialized after election"
        //
        //% Compliance:
        //% `nextIndex[]` for each server, index of the next log entry to send to that server
        //% (initialized to leader last log index + 1)
        //
        //% Compliance:
        //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server
        //% (initialized to 0, increases monotonically)
        let leader_last_idx_plus_one = raft_state.log.last_idx() + 1;
        self.next_idx
            .iter_mut()
            .for_each(|(_peer_id, idx)| *idx = leader_last_idx_plus_one);
        self.match_idx
            .iter_mut()
            .for_each(|(_peer_id, idx)| *idx = Idx::initial());

        //% Compliance:
        //% Upon election: send initial empty AppendEntries RPCs (heartbeat) to each server; repeat
        //% during idle periods to prevent election timeouts (§5.2)
        self.broadcast_send_append_entries(server_id, peer_list, raft_state, io_egress);
    }

    fn broadcast_send_append_entries<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerId],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        for peer_id in peer_list.iter() {
            self.on_send_append_entry(server_id, peer_id, raft_state, io_egress);
        }
    }

    fn on_send_append_entry<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_id: &PeerId,
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        let leader_current_term = raft_state.current_term;
        let leader_commit_idx = *raft_state.commit_idx();

        let (peer_next_idx, prev_idx) = {
            let peer_next_idx = *self
                .next_idx
                .get(peer_id)
                .expect("peer should have next_idx state");

            //% Compliance:
            //% prevLogIndex: index of log entry immediately preceding new ones
            //% prevLogTerm: term of prevLogIndex entry
            let prev_idx = peer_next_idx - 1;

            (peer_next_idx, prev_idx)
        };

        let prev_log_term_idx = if prev_idx.is_initial() {
            // The peer holds nothing, so the entries are preceded by the empty prefix. Every log
            // contains that prefix, so the peer cannot reject on this.
            TermIdx::initial()
        } else {
            // next_idx is at most last_idx + 1, so prev_idx always names an existing entry.
            let prev_term = raft_state
                .log
                .term_at_idx(&prev_idx)
                .expect("prev_idx is at most last_idx");
            TermIdx::builder().with_term(prev_term).with_idx(prev_idx)
        };

        let entries = raft_state.log.get_entries(&peer_next_idx);

        let rpc = Rpc::new_append_entry(
            leader_current_term,
            *server_id,
            prev_log_term_idx,
            leader_commit_idx,
            entries,
        );

        peer_id.send_rpc(rpc, io_egress);
    }

    pub fn on_timeout<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerId],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        //% Compliance:
        //% Upon election: send initial empty AppendEntries RPCs (heartbeat) to each server; repeat
        //% during idle periods to prevent election timeouts (§5.2)
        self.broadcast_send_append_entries(server_id, peer_list, raft_state, io_egress);
    }

    pub fn on_recv<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_id: PeerId,
        peer_list: &[PeerId],
        rpc: &Rpc,
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        match rpc {
            Rpc::RequestVote(request_vote) => request_vote.on_recv(peer_id, raft_state, io_egress),
            Rpc::RequestVoteResp(_request_vote_resp) => {
                // Ignore since a Leader doesn't send RequestVote
                // TODO: log for observability
            }
            Rpc::AppendEntry(_append_entries) => {
                // Conversion to Follower is already handled so this is simple a sanity check.
                //
                // Raft guarantees that there can only be one elected Leader per term.
                debug_assert!(rpc.term() != &raft_state.current_term);
            }
            Rpc::AppendEntryResp(append_entries_resp) => {
                if let Some(check_match_idx) = self.on_recv_append_entry_resp(
                    server_id,
                    peer_id,
                    append_entries_resp,
                    raft_state,
                    io_egress,
                ) {
                    self.update_commit_idx(check_match_idx, peer_list, raft_state, peer_id);
                }
            }
        }
    }

    //% Compliance:
    //% If there exists an N such that N > commitIndex, a majority of matchIndex[i] ≥ N,
    //% and log[N].term == currentTerm: set commitIndex = N (§5.3, §5.4).
    fn update_commit_idx(
        &mut self,
        newly_inserted_match_idx: Idx,
        peer_list: &[PeerId],
        raft_state: &mut RaftState,
        peer_id: PeerId,
    ) {
        //% Compliance:
        //% N > commitIndex
        let larger_than_current_commit_idx = &newly_inserted_match_idx > raft_state.commit_idx();

        let new_idx_larger_than_majority = {
            let larger_match_idx_count = self
                .match_idx
                .iter()
                .filter(|(_peer_id, peer_match_idx)| {
                    //% Compliance:
                    //% matchIndex[i] ≥ N
                    peer_match_idx >= &&newly_inserted_match_idx
                })
                .count();
            //% Compliance:
            //% majority
            //
            // The Leader counts itself toward the quorum since it trivially has every entry in
            // its own log; match_idx only tracks peers, so add 1 for the Leader.
            larger_match_idx_count + 1 >= Mode::quorum(peer_list)
        };

        //% Compliance:
        //% log[N].term == currentTerm
        let matches_current_term = raft_state
            .log
            .term_at_idx(&newly_inserted_match_idx)
            .map(|term| term.eq(&raft_state.current_term))
            .is_some_and(|matches| matches);

        if larger_than_current_commit_idx && new_idx_larger_than_majority && matches_current_term {
            //% Compliance:
            //% set commitIndex = N (§5.3, §5.4).
            raft_state.set_commit_idx(newly_inserted_match_idx, peer_id, CurrentMode::Leader);
        }
    }

    // Echoed Idx from the received AppendEntriesResp.
    //
    // Returns None if the RPC was not successful or if the RPC was received out of order (didn't
    // match the peer's next_idx).
    fn on_recv_append_entry_resp<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_id: PeerId,
        append_entries_resp: &AppendEntriesResp,
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) -> Option<Idx> {
        let AppendEntriesResp {
            term: _,
            success,
            echo_prev_log_term_idx,
        } = append_entries_resp;

        // The RPC echoes the prev TermIdx that was sent, which is next_idx - 1.
        let expected_echo_prev_idx = {
            let current_next_idx = *self
                .next_idx
                .get(&peer_id)
                .expect("peer should have next_idx state");

            if current_next_idx.is_initial() {
                Idx::initial()
            } else {
                current_next_idx - 1
            }
        };

        // Only process the response if the RPC matches the current next_idx for the peer. The RPC
        // can be out-of-order due to timeout and re-transmission.
        if expected_echo_prev_idx.eq(&echo_prev_log_term_idx.idx) {
            if *success {
                // Check the TermIdx in the Resp rpc rather than assuming next_idx to make the
                // protocol more resilient.
                let rpc_sent_idx = *self
                    .next_idx
                    .get(&peer_id)
                    .expect("peer should have next_idx state");

                //% Compliance:
                //% If successful: update nextIndex and matchIndex for follower (§5.3)
                self.next_idx
                    .entry(peer_id)
                    .and_modify(|idx| *idx = rpc_sent_idx);
                self.match_idx
                    .entry(peer_id)
                    .and_modify(|idx| *idx = rpc_sent_idx);
                Some(rpc_sent_idx)
            } else {
                //% Compliance:
                //% If AppendEntries fails because of log inconsistency: decrement nextIndex and retry (§5.3)
                self.next_idx.entry(peer_id).and_modify(|idx| {
                    // next_idx bottoms out at 1. The prev TermIdx is then the empty prefix, which
                    // every log matches, so there is nothing left to back off to and Idx(0) names
                    // no entry.
                    debug_assert!(
                        *idx > Idx::from(1),
                        "Peer rejected an initial prev TermIdx, which every log matches."
                    );
                    *idx = idx.sub(1)
                });

                self.on_send_append_entry(server_id, &peer_id, raft_state, io_egress);

                // RPC was not successful.
                None
            }
        } else {
            // RPC was received out of order and didn't match the peer's next_idx.
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        log::{MatchOutcome, Term},
        queue::testing::{helper_inspect_next_sent_packet, MockIo},
        raft_state::RaftState,
        server::{PeerId, ServerId},
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;

    #[tokio::test]
    async fn on_leader() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;
        let mut leader = Leader::new(&peer_list, &mut state);
        assert_eq!(leader.next_idx.get(&peer2_id).unwrap(), &Idx::from(1));
        assert_eq!(leader.next_idx.get(&peer3_id).unwrap(), &Idx::from(1));

        let mut io = MockIo::new(server_id);

        leader.on_leader(&server_id, &peer_list, &mut state, &mut io);

        // Expect append_entry is sent to both peers
        for _ in 0..2 {
            let packet = helper_inspect_next_sent_packet(&mut io);

            // log is empty so expect to recieve a RPC with initial term and idx
            let expected_rpc = Rpc::new_append_entry(
                current_term,
                server_id,
                TermIdx::initial(),
                Idx::initial(),
                vec![],
            );
            assert_eq!(&expected_rpc, packet.rpc());
        }
    }

    #[tokio::test]
    async fn on_leader_with_entries() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;
        let mut leader = Leader::new(&peer_list, &mut state);
        assert_eq!(leader.next_idx.get(&peer2_id).unwrap(), &Idx::from(1));
        assert_eq!(leader.next_idx.get(&peer3_id).unwrap(), &Idx::from(1));

        // Insert two entries into log
        for i in 1..=2 {
            let entry = crate::log::Entry {
                term: current_term,
                command: i,
            };
            let outcome = state
                .log
                .update_to_match_leaders_log(entry.clone(), Idx::from(i as u64));
            assert!(matches!(outcome, MatchOutcome::DoesntExist));
        }

        let mut io = MockIo::new(server_id);

        leader.on_leader(&server_id, &peer_list, &mut state, &mut io);

        // FIXME: need to test sending after the initial on_leader switch
        //
        // // Update next_idx for peer 2 to record that it has received the first entry
        // let peer2_idx = leader.next_idx.get_mut(&peer2_id).unwrap();
        // *peer2_idx += 1;

        let expected_peer_term_idx = vec![
            TermIdx {
                term: current_term,
                idx: Idx::from(2),
            },
            TermIdx {
                term: current_term,
                idx: Idx::from(2),
            },
        ];
        for exptected_term_idx in expected_peer_term_idx {
            // Expect append_entry is sent to both peers
            let packet = helper_inspect_next_sent_packet(&mut io);

            let expected_rpc = Rpc::new_append_entry(
                current_term,
                server_id,
                exptected_term_idx,
                Idx::initial(),
                vec![],
            );
            assert_eq!(&expected_rpc, packet.rpc());

            // TODO: also assert which peer we are sending to once we add peer header info.
        }
    }

    #[tokio::test]
    async fn on_timeout() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;
        let mut leader = Leader::new(&peer_list, &mut state);

        let mut io = MockIo::new(server_id);

        // A timeout triggers a heartbeat (empty AppendEntries) to each peer.
        leader.on_timeout(&server_id, &peer_list, &mut state, &mut io);

        // Expect append_entry is sent to both peers
        for _ in 0..2 {
            let packet = helper_inspect_next_sent_packet(&mut io);

            // log is empty so expect to receive a RPC with initial term and idx
            let expected_rpc = Rpc::new_append_entry(
                current_term,
                server_id,
                TermIdx::initial(),
                Idx::initial(),
                vec![],
            );
            assert_eq!(&expected_rpc, packet.rpc());
        }
    }

    // Verifies that on_timeout sends each peer a heartbeat based on that peer's existing next_idx
    // and, unlike on_leader, does NOT reinitialize next_idx/match_idx.
    #[tokio::test]
    async fn on_timeout_uses_peer_next_idx() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;

        // Insert two entries into log
        for i in 1..=2 {
            let entry = crate::log::Entry {
                term: current_term,
                command: i,
            };
            let outcome = state
                .log
                .update_to_match_leaders_log(entry.clone(), Idx::from(i as u64));
            assert!(matches!(outcome, MatchOutcome::DoesntExist));
        }

        let mut leader = Leader::new(&peer_list, &mut state);
        // next_idx initialized to last_idx + 1 == 3
        assert_eq!(leader.next_idx.get(&peer2_id).unwrap(), &Idx::from(3));
        assert_eq!(leader.next_idx.get(&peer3_id).unwrap(), &Idx::from(3));

        // Record that peer2 is behind (next_idx == 1). Unlike on_leader, on_timeout does not
        // reinitialize next_idx, so the heartbeat should honor this state.
        *leader.next_idx.get_mut(&peer2_id).unwrap() = Idx::from(1);

        let mut io = MockIo::new(server_id);
        leader.on_timeout(&server_id, &peer_list, &mut state, &mut io);

        // next_idx is unchanged after a timeout heartbeat.
        assert_eq!(leader.next_idx.get(&peer2_id).unwrap(), &Idx::from(1));
        assert_eq!(leader.next_idx.get(&peer3_id).unwrap(), &Idx::from(3));

        // prev names the entry immediately preceding the ones being sent, so it is next_idx - 1.
        // The Follower relies on this: it appends the first entry at prev.idx + 1.
        //
        //     Leader log:   Idx:      0        1        2
        //                          [ empty ][  e1  ][  e2  ]
        //                            prefix
        //
        //     peer2, next_idx == 1:    ^        ^
        //                              |        |
        //                        prev == 0    entries start here
        //                     (empty prefix)
        //
        //     peer3, next_idx == 3:                     ^        ^
        //                                               |        |
        //                                         prev == 2    entries start here
        //                                                     (nothing left, so a bare heartbeat)
        //
        // peer2 (next_idx == 1): nothing precedes the first entry, so prev is the empty prefix and
        // the whole log is shipped.
        let expected_peer2 = (
            TermIdx::initial(),
            vec![
                crate::log::Entry::new(current_term, 1),
                crate::log::Entry::new(current_term, 2),
            ],
        );
        // peer3 (next_idx == 3): prev is the last entry in the log and nothing follows it, so this
        // is a bare heartbeat.
        let expected_peer3 = (
            TermIdx {
                term: current_term,
                idx: Idx::from(2),
            },
            vec![],
        );
        for (expected_term_idx, expected_entries) in [expected_peer2, expected_peer3] {
            let packet = helper_inspect_next_sent_packet(&mut io);
            let expected_rpc = Rpc::new_append_entry(
                current_term,
                server_id,
                expected_term_idx,
                Idx::initial(),
                expected_entries,
            );
            assert_eq!(&expected_rpc, packet.rpc());
        }
    }

    #[tokio::test]
    async fn test_on_recv_append_entry_resp() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;

        // Mock sending two AppendEntries (insert two entries into log)
        for i in 1..=2 {
            let entry = crate::log::Entry {
                term: current_term,
                command: i,
            };
            let outcome = state
                .log
                .update_to_match_leaders_log(entry.clone(), Idx::from(i as u64));
            assert!(matches!(outcome, MatchOutcome::DoesntExist));
        }

        let mut leader = Leader::new(&peer_list, &mut state);
        assert_eq!(leader.next_idx.get(&peer2_id).unwrap(), &Idx::from(3));
        assert_eq!(leader.next_idx.get(&peer3_id).unwrap(), &Idx::from(3));
        let mut io = MockIo::new(server_id);

        // RPC where echo idx doesn't match the peer next_idx
        {
            let bad_echo_idx = Idx::from(1);

            let append_entries_resp = AppendEntriesResp {
                term: current_term,
                success: true,
                echo_prev_log_term_idx: TermIdx::builder()
                    .with_term(Term::from(2))
                    .with_idx(bad_echo_idx),
            };
            let idx = leader.on_recv_append_entry_resp(
                &server_id,
                peer2_id,
                &append_entries_resp,
                &mut state,
                &mut io,
            );
            assert!(idx.is_none());
        }

        let echo_idx = Idx::from(2);
        // RPC success: use the correct echo_idx
        {
            let append_entries_resp = AppendEntriesResp {
                term: current_term,
                success: true,
                echo_prev_log_term_idx: TermIdx::builder()
                    .with_term(Term::from(2))
                    .with_idx(echo_idx),
            };
            let idx = leader.on_recv_append_entry_resp(
                &server_id,
                peer2_id,
                &append_entries_resp,
                &mut state,
                &mut io,
            );
            assert_eq!(idx.unwrap(), Idx::from(3));
        }

        // RPC failure: use the same.. now outdated echo idx
        {
            let append_entries_resp = AppendEntriesResp {
                term: current_term,
                success: false,
                echo_prev_log_term_idx: TermIdx::builder()
                    .with_term(Term::from(2))
                    .with_idx(echo_idx),
            };
            let idx = leader.on_recv_append_entry_resp(
                &server_id,
                peer2_id,
                &append_entries_resp,
                &mut state,
                &mut io,
            );
            assert!(idx.is_none());
        }
    }

    // A response is matched to its request by the echoed prev TermIdx, which is next_idx - 1.
    //
    // The Follower copies prev back verbatim, so this is what tells a current response apart from
    // one answering a superseded RPC. Comparing the echo against next_idx itself never matches and
    // discards every response, leaving the Leader unable to advance or to retry a failure.
    //
    //     Leader log:   Idx:      0        1        2
    //                          [ empty ][  e1  ][  e2  ]
    //                    prefix                     ^
    //                                               next_idx == 3
    //                                      ^
    //                                      echo == prev == 2, so this response is current
    #[tokio::test]
    async fn on_recv_append_entry_resp_matches_on_echoed_prev_idx() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;

        for i in 1..=2 {
            let outcome = state.log.update_to_match_leaders_log(
                crate::log::Entry {
                    term: current_term,
                    command: i,
                },
                Idx::from(i as u64),
            );
            assert!(matches!(outcome, MatchOutcome::DoesntExist));
        }

        let mut leader = Leader::new(&peer_list, &mut state);
        assert_eq!(leader.next_idx.get(&peer2_id).unwrap(), &Idx::from(3));
        let mut io = MockIo::new(server_id);

        // A response echoing anything other than next_idx - 1 answers a superseded RPC and is
        // dropped, so next_idx is left alone.
        {
            let append_entries_resp = AppendEntriesResp {
                term: current_term,
                success: false,
                echo_prev_log_term_idx: TermIdx::builder()
                    .with_term(current_term)
                    .with_idx(Idx::from(1)),
            };
            leader.on_recv_append_entry_resp(
                &server_id,
                peer2_id,
                &append_entries_resp,
                &mut state,
                &mut io,
            );
            assert_eq!(leader.next_idx.get(&peer2_id).unwrap(), &Idx::from(3));
        }

        // A response echoing next_idx - 1 is current and is processed. A failure decrements
        // next_idx, which is observable proof the response was not discarded.
        {
            let append_entries_resp = AppendEntriesResp {
                term: current_term,
                success: false,
                echo_prev_log_term_idx: TermIdx::builder()
                    .with_term(current_term)
                    .with_idx(Idx::from(2)),
            };
            let idx = leader.on_recv_append_entry_resp(
                &server_id,
                peer2_id,
                &append_entries_resp,
                &mut state,
                &mut io,
            );

            assert!(idx.is_none());
            assert_eq!(leader.next_idx.get(&peer2_id).unwrap(), &Idx::from(2));
        }
    }

    // next_idx bottoms out at 1, since the prev TermIdx is then the empty prefix that every log
    // matches. A peer that still replies false is malfunctioning and must not walk next_idx into
    // Idx(0), which names no entry.
    //
    //     Idx:      0        1        2        3
    //            [ empty ][  e1  ][  e2  ][  e3  ]
    //              prefix
    //                ^        ^
    //                |        |
    //                |        next_idx == 1, the floor. prev is the empty prefix,
    //                |        which every log matches, so a false reply is bogus.
    //                |
    //                next_idx == 0 is off the front of the log. Nothing to send,
    //                nothing to compare against, and as_log_idx() underflows.
    //
    // Each false reply steps next_idx one slot left. This test starts peer2 on the floor and
    // pushes once more.
    #[should_panic(expected = "Peer rejected an initial prev TermIdx")]
    #[tokio::test]
    async fn on_recv_append_entry_resp_does_not_decrement_next_idx_past_one() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;

        // An empty log puts every peer on the floor, since next_idx starts at last_idx + 1.
        let mut leader = Leader::new(&peer_list, &mut state);
        assert_eq!(leader.next_idx.get(&peer2_id).unwrap(), &Idx::from(1));
        let mut io = MockIo::new(server_id);

        // The RPC sent at next_idx == 1 carries the empty prefix as prev, so that is what a
        // current response echoes back.
        let append_entries_resp = AppendEntriesResp {
            term: current_term,
            success: false,
            echo_prev_log_term_idx: TermIdx::initial(),
        };
        leader.on_recv_append_entry_resp(
            &server_id,
            peer2_id,
            &append_entries_resp,
            &mut state,
            &mut io,
        );
    }

    // A Leader should ignore stale RequestVoteResp from Followers after it has already won the
    // election.
    #[tokio::test]
    async fn leader_ignores_stale_request_vote_resp() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;
        let mut leader = Leader::new(&peer_list, &mut state);
        let mut io = MockIo::new(server_id);

        let next_idx_before = leader.next_idx.clone();
        let match_idx_before = leader.match_idx.clone();

        // A stale vote (term == current_term so Mode wouldn't have converted us to Follower).
        let stale_vote = Rpc::new_request_vote_resp(current_term, true);
        leader.on_recv(
            &server_id,
            peer2_id,
            &peer_list,
            &stale_vote,
            &mut state,
            &mut io,
        );

        // The Leader neither replies nor mutates any state.
        assert!(io.send_queue.is_empty());
        assert_eq!(leader.next_idx, next_idx_before);
        assert_eq!(leader.match_idx, match_idx_before);
        assert_eq!(state.current_term, current_term);
    }

    // Verifies commitIdx only advances once a majority of matchIndex[] have reached N (§5.3, §5.4).
    #[tokio::test]
    async fn update_commit_idx_requires_majority() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;

        // 3-node cluster (2 peers + self) => quorum of 2.
        // Insert one entry at the current term.
        let entry = crate::log::Entry {
            term: current_term,
            command: 1,
        };
        let outcome = state.log.update_to_match_leaders_log(entry, Idx::from(1));
        assert!(matches!(outcome, MatchOutcome::DoesntExist));

        let mut leader = Leader::new(&peer_list, &mut state);
        // match_idx initialized to 0 for each peer.
        assert_eq!(state.commit_idx(), &Idx::initial());

        // No peer has replicated idx 1: only the Leader has it (1 of 3) which is short of the
        // quorum of 2, so commit_idx does not advance.
        {
            leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);
            assert_eq!(state.commit_idx(), &Idx::initial());
        }

        // One peer has replicated idx 1: together with the Leader that is a majority (2 of 3), so
        // commit_idx advances to 1. The Leader counts itself toward the quorum.
        {
            *leader.match_idx.get_mut(&peer2_id).unwrap() = Idx::from(1);
            leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);
            assert_eq!(state.commit_idx(), &Idx::from(1));
        }

        // N is not greater than the current commit_idx: no change.
        {
            leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);
            assert_eq!(state.commit_idx(), &Idx::from(1));
        }
    }

    // Test quorum size for a 3 node cluster.
    #[tokio::test]
    async fn update_commit_idx_leader_counts_itself_3_node() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];

        // Boundary: exactly `peers_at_n` peers have replicated idx 1. `expect_commit` is the
        // correct Raft outcome once the Leader is counted (leader + peers_at_n >= quorum).
        for (peers_at_n, expect_commit) in [(0, false), (1, true)] {
            let mut state = RaftState::new(timeout.clone());
            let entry = crate::log::Entry {
                term: state.current_term,
                command: 1,
            };
            let _ = state.log.update_to_match_leaders_log(entry, Idx::from(1));

            let mut leader = Leader::new(&peer_list, &mut state);
            for peer_id in peer_list.iter().take(peers_at_n) {
                *leader.match_idx.get_mut(peer_id).unwrap() = Idx::from(1);
            }

            leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);

            let expected = if expect_commit {
                Idx::from(1)
            } else {
                Idx::initial()
            };
            assert_eq!(
                state.commit_idx(),
                &expected,
                "3-node cluster with {peers_at_n} replicating peer(s)"
            );
        }
    }

    // Verifies a Leader never commits an entry from a previous term, even with a majority (§5.4.2).
    #[tokio::test]
    async fn update_commit_idx_only_commits_current_term() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);

        // Insert an entry from an older term, then advance the current term.
        let old_term = state.current_term;
        let entry = crate::log::Entry {
            term: old_term,
            command: 1,
        };
        let outcome = state.log.update_to_match_leaders_log(entry, Idx::from(1));
        assert!(matches!(outcome, MatchOutcome::DoesntExist));
        state.current_term = Term::from(2);

        let mut leader = Leader::new(&peer_list, &mut state);

        // Both peers have replicated idx 1 (a majority), but log[1].term != currentTerm so the
        // Leader must NOT advance commit_idx.
        *leader.match_idx.get_mut(&peer2_id).unwrap() = Idx::from(1);
        *leader.match_idx.get_mut(&peer3_id).unwrap() = Idx::from(1);
        leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);
        assert_eq!(state.commit_idx(), &Idx::initial());
    }
}
