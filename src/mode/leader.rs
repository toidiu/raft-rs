use crate::{
    io::ServerEgress,
    log::{Idx, TermIdx},
    mode::Mode,
    packet::{AppendEntriesResp, Rpc},
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

        let last_log_term_idx = raft_state.log.last_term_idx();
        let peer_next_idx = *self
            .next_idx
            .get(peer_id)
            .expect("peer should have next_idx state");

        //% Compliance:
        //% If last log index ≥ nextIndex for a follower: send AppendEntries RPC with log
        //% entries starting at nextIndex
        let peer_term_idx = if last_log_term_idx.idx >= peer_next_idx {
            let peer_next_term = raft_state.log.term_at_idx(&peer_next_idx).unwrap();
            TermIdx::builder()
                .with_term(peer_next_term)
                .with_idx(peer_next_idx)
        } else {
            // The peer should be theoritically up-to-date so send the latest Leader entry. If
            // the peer is not up-to-date we will get a failure and will decrement the peer's
            // nextIndex.
            last_log_term_idx
        };

        let rpc = Rpc::new_append_entry(
            leader_current_term,
            *server_id,
            peer_term_idx,
            leader_commit_idx,
            vec![],
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
                debug_assert!(false);
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
    pub(crate) fn update_commit_idx(
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
            larger_match_idx_count >= Mode::quorum(peer_list)
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

        let current_next_idx = *self
            .next_idx
            .get(&peer_id)
            .expect("peer should have next_idx state");

        // Only process the response if the RPC matches the current next_idx for the peer. The RPC
        // can be out-of-order due to timeout and re-transmission.
        if current_next_idx.eq(&echo_prev_log_term_idx.idx) {
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
                    assert!(
                        !idx.is_initial(),
                        "Peer responded false to initial Idx, which is malformed behavior."
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
        io::testing::{helper_inspect_next_sent_packet, MockIo},
        log::{MatchOutcome, Term},
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
            let append_entries_resp = AppendEntriesResp {
                term: current_term,
                success: true,
                echo_prev_log_term_idx: TermIdx::builder()
                    .with_term(Term::from(2))
                    .with_idx(Idx::from(1)),
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

        // RPC success
        {
            let append_entries_resp = AppendEntriesResp {
                term: current_term,
                success: true,
                echo_prev_log_term_idx: TermIdx::builder()
                    .with_term(Term::from(2))
                    .with_idx(Idx::from(3)),
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

        // RPC failure
        {
            let append_entries_resp = AppendEntriesResp {
                term: current_term,
                success: false,
                echo_prev_log_term_idx: TermIdx::builder()
                    .with_term(Term::from(2))
                    .with_idx(Idx::from(3)),
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

    // ==================== on_timeout tests ====================

    #[tokio::test]
    async fn ai_test_on_timeout_sends_heartbeats_to_all_peers() {
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

        // Call on_timeout
        leader.on_timeout(&server_id, &peer_list, &mut state, &mut io);

        // Expect heartbeats sent to both peers
        for _ in 0..2 {
            let packet = helper_inspect_next_sent_packet(&mut io);
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
    async fn ai_test_on_timeout_empty_peer_list() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer_list: Vec<PeerId> = vec![];
        let mut state = RaftState::new(timeout);
        let mut leader = Leader::new(&peer_list, &mut state);
        let mut io = MockIo::new(server_id);

        // Should not panic with empty peer list
        leader.on_timeout(&server_id, &peer_list, &mut state, &mut io);

        // No packets should be sent
        assert!(io.send_queue.is_empty());
    }

    #[tokio::test]
    async fn ai_test_on_timeout_with_empty_log() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer_list = vec![peer2_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;
        let mut leader = Leader::new(&peer_list, &mut state);
        let mut io = MockIo::new(server_id);

        // Verify log is empty
        assert!(state.log.is_empty());

        leader.on_timeout(&server_id, &peer_list, &mut state, &mut io);

        // With empty log, should send TermIdx::initial()
        let packet = helper_inspect_next_sent_packet(&mut io);
        let expected_rpc = Rpc::new_append_entry(
            current_term,
            server_id,
            TermIdx::initial(),
            Idx::initial(),
            vec![],
        );
        assert_eq!(&expected_rpc, packet.rpc());
    }

    #[tokio::test]
    async fn ai_test_on_timeout_with_log_entries() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer_list = vec![peer2_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;

        // Insert entries into log before creating leader
        for i in 1..=3 {
            let entry = crate::log::Entry {
                term: current_term,
                command: i,
            };
            let _ = state
                .log
                .update_to_match_leaders_log(entry, Idx::from(i as u64));
        }

        let mut leader = Leader::new(&peer_list, &mut state);
        let mut io = MockIo::new(server_id);

        // next_idx should be initialized to last_idx + 1 = 4
        assert_eq!(leader.next_idx.get(&peer2_id).unwrap(), &Idx::from(4));

        leader.on_timeout(&server_id, &peer_list, &mut state, &mut io);

        // Should send with last_log_term_idx since peer is theoretically up-to-date
        let packet = helper_inspect_next_sent_packet(&mut io);
        let expected_rpc = Rpc::new_append_entry(
            current_term,
            server_id,
            TermIdx::builder()
                .with_term(current_term)
                .with_idx(Idx::from(3)),
            Idx::initial(),
            vec![],
        );
        assert_eq!(&expected_rpc, packet.rpc());
    }

    #[tokio::test]
    async fn ai_test_on_timeout_multiple_calls() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer_list = vec![peer2_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;
        let mut leader = Leader::new(&peer_list, &mut state);
        let mut io = MockIo::new(server_id);

        // Call on_timeout multiple times
        for _ in 0..3 {
            leader.on_timeout(&server_id, &peer_list, &mut state, &mut io);
        }

        // Should have 3 heartbeats in the queue
        for _ in 0..3 {
            let packet = helper_inspect_next_sent_packet(&mut io);
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

    // ==================== update_commit_idx tests ====================

    #[tokio::test]
    async fn ai_test_update_commit_idx_advances_when_all_conditions_met() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let _server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;

        // Insert entry with current term
        let entry = crate::log::Entry {
            term: current_term,
            command: 1,
        };
        let _ = state.log.update_to_match_leaders_log(entry, Idx::from(1));

        let mut leader = Leader::new(&peer_list, &mut state);

        // Set match_idx for both peers to 1 (quorum = 2 for 3-node cluster)
        leader.match_idx.insert(peer2_id, Idx::from(1));
        leader.match_idx.insert(peer3_id, Idx::from(1));

        // Initial commit_idx should be 0
        assert_eq!(state.commit_idx(), &Idx::initial());

        // Call update_commit_idx
        leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);

        // commit_idx should advance to 1
        assert_eq!(state.commit_idx(), &Idx::from(1));
    }

    #[tokio::test]
    async fn ai_test_update_commit_idx_no_advance_when_idx_lte_commit_idx() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let _server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;

        // Insert two entries
        for i in 1..=2 {
            let entry = crate::log::Entry {
                term: current_term,
                command: i,
            };
            let _ = state
                .log
                .update_to_match_leaders_log(entry, Idx::from(i as u64));
        }

        let mut leader = Leader::new(&peer_list, &mut state);

        // Set match_idx for quorum at index 1
        leader.match_idx.insert(peer2_id, Idx::from(1));
        leader.match_idx.insert(peer3_id, Idx::from(1));

        // Advance commit_idx to 1 first
        leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);
        assert_eq!(state.commit_idx(), &Idx::from(1));

        // Try to update with idx == current commit_idx (should not advance)
        leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);

        // commit_idx should remain at 1
        assert_eq!(state.commit_idx(), &Idx::from(1));
    }

    #[tokio::test]
    async fn ai_test_update_commit_idx_no_advance_without_quorum() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let _server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;

        // Insert entry
        let entry = crate::log::Entry {
            term: current_term,
            command: 1,
        };
        let _ = state.log.update_to_match_leaders_log(entry, Idx::from(1));

        let mut leader = Leader::new(&peer_list, &mut state);

        // Only one peer has match_idx = 1 (not enough for quorum of 2)
        leader.match_idx.insert(peer2_id, Idx::from(1));
        leader.match_idx.insert(peer3_id, Idx::initial());

        leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);

        // commit_idx should NOT advance (only 1 peer, need 2 for quorum)
        assert_eq!(state.commit_idx(), &Idx::initial());
    }

    #[tokio::test]
    async fn ai_test_update_commit_idx_no_advance_when_term_mismatch() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let _server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);

        // Insert entry with a PREVIOUS term (not current term)
        let previous_term = Term::from(1);
        let entry = crate::log::Entry {
            term: previous_term,
            command: 1,
        };
        let _ = state.log.update_to_match_leaders_log(entry, Idx::from(1));

        // Advance to a new term
        state.current_term = Term::from(2);

        let mut leader = Leader::new(&peer_list, &mut state);

        // Both peers have match_idx = 1 (quorum satisfied)
        leader.match_idx.insert(peer2_id, Idx::from(1));
        leader.match_idx.insert(peer3_id, Idx::from(1));

        leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);

        // commit_idx should NOT advance - entry term (1) != current term (2)
        // This is a critical Raft safety guarantee (§5.4)
        assert_eq!(state.commit_idx(), &Idx::initial());
    }

    #[tokio::test]
    async fn ai_test_update_commit_idx_no_advance_when_entry_not_in_log() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let _server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);

        // Log is empty - no entries
        assert!(state.log.is_empty());

        let mut leader = Leader::new(&peer_list, &mut state);

        // Set match_idx to an index that doesn't exist in log
        leader.match_idx.insert(peer2_id, Idx::from(5));
        leader.match_idx.insert(peer3_id, Idx::from(5));

        leader.update_commit_idx(Idx::from(5), &peer_list, &mut state, peer2_id);

        // commit_idx should NOT advance - entry at idx 5 doesn't exist
        assert_eq!(state.commit_idx(), &Idx::initial());
    }

    #[tokio::test]
    async fn ai_test_update_commit_idx_3_node_cluster_quorum() {
        // 3-node cluster: needs 2 for quorum (majority of 3)
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let _server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer_list = vec![peer2_id, peer3_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;

        // Insert entry
        let entry = crate::log::Entry {
            term: current_term,
            command: 1,
        };
        let _ = state.log.update_to_match_leaders_log(entry, Idx::from(1));

        let mut leader = Leader::new(&peer_list, &mut state);

        // Exactly 2 peers with match_idx >= 1 (meets quorum of 2)
        leader.match_idx.insert(peer2_id, Idx::from(1));
        leader.match_idx.insert(peer3_id, Idx::from(1));

        leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);

        // commit_idx should advance
        assert_eq!(state.commit_idx(), &Idx::from(1));
    }

    #[tokio::test]
    async fn ai_test_update_commit_idx_5_node_cluster_quorum() {
        // 5-node cluster: needs 3 for quorum (majority of 5)
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let _server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer3_id = PeerId::new([12; 16]);
        let peer4_id = PeerId::new([13; 16]);
        let peer5_id = PeerId::new([14; 16]);
        let peer_list = vec![peer2_id, peer3_id, peer4_id, peer5_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;

        // Insert entry
        let entry = crate::log::Entry {
            term: current_term,
            command: 1,
        };
        let _ = state.log.update_to_match_leaders_log(entry, Idx::from(1));

        let mut leader = Leader::new(&peer_list, &mut state);

        // Only 2 peers have match_idx = 1 (not enough, need 3)
        leader.match_idx.insert(peer2_id, Idx::from(1));
        leader.match_idx.insert(peer3_id, Idx::from(1));
        leader.match_idx.insert(peer4_id, Idx::initial());
        leader.match_idx.insert(peer5_id, Idx::initial());

        leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);

        // commit_idx should NOT advance (only 2 peers, need 3)
        assert_eq!(state.commit_idx(), &Idx::initial());

        // Now add third peer to quorum
        leader.match_idx.insert(peer4_id, Idx::from(1));

        leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer4_id);

        // commit_idx should now advance (3 peers meet quorum)
        assert_eq!(state.commit_idx(), &Idx::from(1));
    }

    #[tokio::test]
    async fn ai_test_update_commit_idx_single_peer_cluster() {
        // 2-node cluster (1 peer + self): quorum = 2
        // Note: Leader doesn't count itself in match_idx, so needs 2 peers
        // But with only 1 peer in peer_list, quorum calculation is:
        // quorum(peer_list) = (1 + 1) / 2 + 1 = 2
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let _server_id = ServerId::new([1; 16]);
        let peer2_id = PeerId::new([11; 16]);
        let peer_list = vec![peer2_id];
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;

        // Insert entry
        let entry = crate::log::Entry {
            term: current_term,
            command: 1,
        };
        let _ = state.log.update_to_match_leaders_log(entry, Idx::from(1));

        let mut leader = Leader::new(&peer_list, &mut state);

        // Single peer has match_idx = 1
        // But quorum = 2 for 2-node cluster, and we only track 1 peer in match_idx
        // This means 1 peer is not enough to meet quorum of 2
        leader.match_idx.insert(peer2_id, Idx::from(1));

        leader.update_commit_idx(Idx::from(1), &peer_list, &mut state, peer2_id);

        // With quorum = 2 and only 1 peer tracked, commit should NOT advance
        // unless the implementation counts the leader as implicitly having the entry
        // Based on the current implementation, it only counts match_idx entries
        assert_eq!(state.commit_idx(), &Idx::initial());
    }
}
