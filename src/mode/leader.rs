use crate::{
    io::ServerEgress,
    log::{Idx, TermIdx},
    raft_state::RaftState,
    rpc::Rpc,
    server::{PeerInfo, ServerId},
};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct Leader {
    // ==== Volatile state on leaders ====
    //% Compliance:
    //% `nextIndex[]` for each server, index of the next log entry to send to that server
    //% (initialized to leader last log index + 1)
    pub next_idx: BTreeMap<ServerId, Idx>,

    //% Compliance:
    //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server
    //% (initialized to 0, increases monotonically)
    pub match_idx: BTreeMap<ServerId, Idx>,
}

impl Leader {
    pub fn new(peer_list: &[PeerInfo], raft_state: &mut RaftState) -> Self {
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
            next_idx.insert(peer.id, next_log_idx);
            match_idx.insert(peer.id, initial_idx);
        }

        Leader {
            next_idx,
            match_idx,
        }
    }

    pub fn on_leader<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerInfo],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        //% Compliance:
        //% Upon election: send initial empty AppendEntries RPCs (heartbeat) to each server; repeat
        //% during idle periods to prevent election timeouts (§5.2)
        self.on_send_append_entry(server_id, peer_list, raft_state, io_egress);
    }

    fn on_send_append_entry<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerInfo],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        let leader_current_term = raft_state.current_term;
        let leader_commit_idx = raft_state.commit_idx;

        for peer in peer_list.iter() {
            let last_log_term_idx = raft_state.log.last_term_idx();
            let peer_next_idx = *self.next_idx.get(&peer.id).unwrap();

            //% Compliance:
            //% If last log index ≥ nextIndex for a follower: send AppendEntries RPC with log
            //% entries starting at nextIndex
            let peer_term_idx = if last_log_term_idx.idx >= peer_next_idx {
                let peer_next_term = raft_state.log.term_at_idx(&peer_next_idx).unwrap();
                TermIdx::builder()
                    .with_term(peer_next_term)
                    .with_idx(peer_next_idx)
            } else {
                // TODO: this is used as a heartbeat but its not clear what to do here
                //
                // This should only happen right after a server becomes a Leader
                last_log_term_idx
            };

            let rpc = Rpc::new_append_entry(
                leader_current_term,
                *server_id,
                peer_term_idx,
                leader_commit_idx,
                vec![],
            );

            peer.send_rpc(rpc, io_egress);
        }
    }

    pub fn on_timeout<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerInfo],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        //% Compliance:
        //% Upon election: send initial empty AppendEntries RPCs (heartbeat) to each server; repeat
        //% during idle periods to prevent election timeouts (§5.2)
        self.on_send_append_entry(server_id, peer_list, raft_state, io_egress);
    }

    pub fn on_recv(&mut self, _rpc: crate::rpc::Rpc) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        io::testing::MockIo,
        log::MatchOutcome,
        raft_state::RaftState,
        server::{PeerInfo, ServerId},
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;
    use s2n_codec::DecoderBuffer;

    #[tokio::test]
    async fn on_leader() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = ServerId::new([2; 16]);
        let peer3_id = ServerId::new([3; 16]);
        let peer_list = PeerInfo::mock_list(&[peer2_id, peer3_id]);
        let mut state = RaftState::new(timeout);
        let current_term = state.current_term;
        let mut leader = Leader::new(&peer_list, &mut state);

        let mut io = MockIo::new();

        leader.on_leader(&server_id, &peer_list, &mut state, &mut io);

        // Expect append_entry is sent to both peers
        for _ in 0..2 {
            let rpc_bytes = io.send_queue.pop_front().unwrap();
            let buffer = DecoderBuffer::new(&rpc_bytes);
            let (sent_rpc, _buffer) = buffer.decode::<Rpc>().unwrap();

            // log is empty so expect to recieve a RPC with initial term and idx
            let expected_rpc = Rpc::new_append_entry(
                current_term,
                server_id,
                TermIdx::initial(),
                Idx::initial(),
                vec![],
            );
            assert_eq!(sent_rpc, expected_rpc);
        }
    }

    #[tokio::test]
    async fn on_leader_with_entries() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer2_id = ServerId::new([2; 16]);
        let peer3_id = ServerId::new([3; 16]);
        let peer_list = PeerInfo::mock_list(&[peer2_id, peer3_id]);
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
                .match_leaders_log(entry.clone(), Idx::from(i as u64));
            assert!(matches!(outcome, MatchOutcome::DoesntExist));
        }

        // Update next_idx for peer 2 to record that it has received the first entry
        let peer2_idx = leader.next_idx.get_mut(&peer2_id).unwrap();
        *peer2_idx += 1;

        let mut io = MockIo::new();
        leader.on_leader(&server_id, &peer_list, &mut state, &mut io);

        let expected_peer_term_idx = vec![
            TermIdx {
                term: current_term,
                idx: Idx::from(2),
            },
            TermIdx {
                term: current_term,
                idx: Idx::from(1),
            },
        ];
        for exptected_term_idx in expected_peer_term_idx {
            // Expect append_entry is sent to both peers
            let rpc_bytes = io.send_queue.pop_front().unwrap();
            let buffer = DecoderBuffer::new(&rpc_bytes);
            let (sent_rpc, _buffer) = buffer.decode::<Rpc>().unwrap();
            let expected_rpc = Rpc::new_append_entry(
                current_term,
                server_id,
                exptected_term_idx,
                Idx::initial(),
                vec![],
            );
            assert_eq!(sent_rpc, expected_rpc);
        }
    }
}
