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
        self.on_broadcast_send_append_entries(server_id, peer_list, raft_state, io_egress);
    }

    fn on_broadcast_send_append_entries<E: ServerEgress>(
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
        self.on_broadcast_send_append_entries(server_id, peer_list, raft_state, io_egress);
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
                return Some(rpc_sent_idx);
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
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        io::testing::{helper_inspect_next_sent_packet, MockIo},
        log::MatchOutcome,
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

            // TODO: aslo assert which peer we are sending to once we add peer header info.
        }
    }
}
