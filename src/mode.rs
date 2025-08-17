//! Raft state diagram.
//!
//! 1: startup
//! 2: timeout. start election
//! 3: timeout. new election
//! 4: recv vote from majority of servers
//! 5: discover current leader or new term
//! 6: discover server with higher term
//!
//!
//! ```none
//!
//!     |                       ------
//!     | 1                    |  3   |
//!     v             2        |      v
//! +----------+ --------->  +-----------+
//! |          |             |           |
//! | Follower |             | Candidate |
//! |          |             |           |
//! +----------+  <--------- +-----------+
//!        ^          5             |
//!        |                        | 4
//!        |                        v
//!        |          6        +--------+
//!         ------------------ |        |
//!                            | Leader |
//!                            |        |
//!                            +--------+
//!
//! ```
//! https://textik.com/#8dbf6540e0dd1676

use crate::{
    io::ServerEgress,
    macros::cast_unsafe,
    mode::{candidate::Candidate, follower::Follower, leader::Leader},
    raft_state::RaftState,
    rpc::Rpc,
    server::{PeerInfo, ServerId},
};

mod candidate;
mod follower;
mod leader;

pub enum Mode {
    Follower(Follower),
    Candidate(Candidate),
    Leader(Leader),
}

impl Mode {
    pub fn new() -> Self {
        Mode::Follower(Follower)
    }

    pub fn on_timeout<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerInfo],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        match self {
            Mode::Follower(follower) => {
                let transition = follower.on_timeout();
                self.handle_mode_transition(
                    server_id, peer_list, transition, raft_state, io_egress,
                );
            }
            Mode::Candidate(candidate) => {
                let transition = candidate.on_timeout(server_id, peer_list, raft_state, io_egress);
                self.handle_mode_transition(
                    server_id, peer_list, transition, raft_state, io_egress,
                );
            }

            Mode::Leader(leader) => leader.on_timeout(server_id, peer_list, raft_state, io_egress),
        }
    }

    pub fn on_recv<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_id: ServerId,
        rpc: Rpc,
        peer_list: &[PeerInfo],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        //% Compliance:
        //% If RPC request or response contains term T > currentTerm: set currentTerm = T, convert
        //% to follower (§5.1)
        if rpc.term() > &raft_state.current_term {
            raft_state.current_term = *rpc.term();
            self.on_follower();
        }

        let process_rpc_again = match self {
            Mode::Follower(follower) => {
                //% Compliance:
                //% If election timeout elapses without receiving AppendEntries RPC from current
                //% leader or granting vote to candidate: convert to candidate
                follower.on_recv(rpc, raft_state, io_egress);
                None
            }
            Mode::Candidate(candidate) => {
                let (transition, rpc) =
                    candidate.on_recv(peer_id, rpc, peer_list, raft_state, io_egress);
                self.handle_mode_transition(
                    server_id, peer_list, transition, raft_state, io_egress,
                );
                rpc
            }
            Mode::Leader(leader) => {
                leader.on_recv(rpc);
                None
            }
        };

        // Attempt to process the RPC again.
        //
        // An RPC might only be partially processed if it results in a ModeTransition and should be
        // processed again by the new Mode.
        if let Some(rpc) = process_rpc_again {
            self.on_recv(server_id, peer_id, rpc, peer_list, raft_state, io_egress)
        }
    }

    fn handle_mode_transition<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerInfo],
        transition: ModeTransition,
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        match transition {
            ModeTransition::Noop => (),
            ModeTransition::ToFollower => self.on_follower(),
            ModeTransition::ToCandidate => {
                self.on_candidate(server_id, peer_list, raft_state, io_egress)
            }
            ModeTransition::ToLeader => self.on_leader(server_id, peer_list, raft_state, io_egress),
        }
    }

    fn on_follower(&mut self) {
        *self = Mode::Follower(Follower);
        let follower = cast_unsafe!(self, Mode::Follower);
        follower.on_follower();
    }

    fn on_candidate<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerInfo],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        *self = Mode::Candidate(Candidate::default());
        let candidate = cast_unsafe!(self, Mode::Candidate);

        match candidate.on_candidate(server_id, peer_list, raft_state, io_egress) {
            ModeTransition::Noop => (),
            ModeTransition::ToLeader => {
                // If the quorum size is 1, then a candidate will become leader immediately
                self.on_leader(server_id, peer_list, raft_state, io_egress);
            }
            ModeTransition::ToCandidate | ModeTransition::ToFollower => {
                unreachable!("Invalid mode transition");
            }
        }
    }

    fn on_leader<E: ServerEgress>(
        &mut self,
        server_id: &ServerId,
        peer_list: &[PeerInfo],
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        let leader = Leader::new(peer_list, raft_state);
        *self = Mode::Leader(leader);
        let leader = cast_unsafe!(self, Mode::Leader);
        leader.on_leader(server_id, peer_list, raft_state, io_egress);
    }

    fn quorum(peer_list: &[PeerInfo]) -> usize {
        let peer_plus_self = peer_list.len() + 1;
        let half = peer_plus_self / 2;
        half + 1
    }
}

#[must_use]
pub enum ModeTransition {
    Noop,
    ToFollower,
    ToCandidate,
    ToLeader,
}

#[must_use]
pub enum ElectionResult {
    Elected,
    Pending,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        io::testing::{helper_inspect_sent_rpc, MockIo},
        log::{Idx, Term, TermIdx},
        server::PeerInfo,
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;

    #[tokio::test]
    async fn test_quorum() {
        let peer2_id = ServerId::new([2; 16]);
        let peer3_id = ServerId::new([3; 16]);
        let peer4_id = ServerId::new([4; 16]);

        let peer_list = PeerInfo::mock_list(&[]);
        assert_eq!(Mode::quorum(&peer_list), 1);

        let peer_list = PeerInfo::mock_list(&[peer2_id]);
        assert_eq!(Mode::quorum(&peer_list), 2);

        let peer_list = PeerInfo::mock_list(&[peer2_id, peer3_id]);
        assert_eq!(Mode::quorum(&peer_list), 2);

        let peer_list = PeerInfo::mock_list(&[peer2_id, peer3_id, peer4_id]);
        assert_eq!(Mode::quorum(&peer_list), 3);
    }

    #[tokio::test]
    // recv append_entries with gt or eq term
    async fn candidate_stays_candidate_on_recv_append_entries() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let leader_id = ServerId::new([2; 16]);
        let peer_list = PeerInfo::mock_list(&[leader_id]);
        let mut state = RaftState::new(timeout);
        let current_term = Term::from(2);
        state.current_term = current_term;

        let mut mode = Mode::Candidate(Candidate::default());

        // Mock send AppendEntries to Candidate with `term => current_term`
        let append_entries = Rpc::new_append_entry(
            current_term,
            leader_id,
            TermIdx::initial(),
            Idx::initial(),
            vec![],
        );
        let mut leader_io = MockIo::new();
        mode.on_recv(
            &server_id,
            leader_id,
            append_entries,
            &peer_list,
            &mut state,
            &mut leader_io,
        );

        // expect Mode::Follower
        assert!(matches!(mode, Mode::Follower(_)));

        // decode the sent RPC
        let sent_request_vote = helper_inspect_sent_rpc(&mut leader_io);
        assert!(leader_io.send_queue.is_empty());

        // expect Follower to send RespAppendEntries acknowledging the leader
        // construct RPC to compare
        let expected_rpc = Rpc::new_append_entry_resp(current_term, true);
        assert_eq!(expected_rpc, sent_request_vote);
    }

    #[tokio::test]
    // recv append_entries with smaller term
    async fn candidate_switches_to_follower_on_recv_append_entries() {
        let current_term = Term::from(2);

        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let server_id = ServerId::new([1; 16]);
        let peer_id = ServerId::new([2; 16]);
        let peer_list = PeerInfo::mock_list(&[peer_id]);
        let mut state = RaftState::new(timeout);
        state.current_term = current_term;

        let mut io = MockIo::new();
        let mut mode = Mode::Candidate(Candidate::default());

        // Mock send AppendEntries to Candidate with `term => current_term`
        let append_entries = Rpc::new_append_entry(
            Term::from(1),
            peer_id,
            TermIdx::initial(),
            Idx::initial(),
            vec![],
        );
        mode.on_recv(
            &server_id,
            peer_id,
            append_entries,
            &peer_list,
            &mut state,
            &mut io,
        );

        // expect Mode::Candidate
        assert!(matches!(mode, Mode::Candidate(_)));

        // decode the sent RPC
        assert!(io.send_queue.len() == 1);
        let sent_request_vote = helper_inspect_sent_rpc(&mut io);

        // expect Follower to send RespAppendEntries acknowledging the leader
        let expected_rpc = Rpc::new_append_entry_resp(current_term, false);
        assert_eq!(expected_rpc, sent_request_vote);
    }
}
