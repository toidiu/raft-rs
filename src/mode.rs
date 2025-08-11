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
    rpc::Rpc,
    server::{Context, ServerId},
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
    fn on_timeout<E: ServerEgress>(&mut self, context: &mut Context, io_egress: &mut E) {
        match self {
            Mode::Follower(follower) => {
                let transition = follower.on_timeout();
                self.handle_mode_transition(transition, context, io_egress);
            }
            Mode::Candidate(candidate) => {
                let transition = candidate.on_timeout(context, io_egress);
                self.handle_mode_transition(transition, context, io_egress);
            }

            Mode::Leader(leader) => leader.on_timeout(),
        }
    }

    fn on_recv<E: ServerEgress>(
        &mut self,
        peer_id: ServerId,
        rpc: Rpc,
        context: &mut Context,
        io_egress: &mut E,
    ) {
        //% Compliance:
        //% If RPC request or response contains term T > currentTerm: set currentTerm = T, convert
        //% to follower (§5.1)
        if rpc.term() > &context.raft_state.current_term {
            context.raft_state.current_term = *rpc.term();
            self.on_follower(context);
        }

        let process_rpc_again = match self {
            Mode::Follower(follower) => {
                //% Compliance:
                //% If election timeout elapses without receiving AppendEntries RPC from current
                //% leader or granting vote to candidate: convert to candidate
                follower.on_recv(rpc, context, io_egress);
                None
            }
            Mode::Candidate(candidate) => {
                let (transition, rpc) = candidate.on_recv(peer_id, rpc, context, io_egress);
                self.handle_mode_transition(transition, context, io_egress);
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
            self.on_recv(peer_id, rpc, context, io_egress)
        }
    }

    fn handle_mode_transition<E: ServerEgress>(
        &mut self,
        transition: ModeTransition,
        context: &mut Context,
        io_egress: &mut E,
    ) {
        match transition {
            ModeTransition::None => (),
            ModeTransition::ToFollower => self.on_follower(context),
            ModeTransition::ToCandidate => self.on_candidate(context, io_egress),
            ModeTransition::ToLeader => self.on_leader(),
        }
    }

    fn on_follower(&mut self, context: &mut Context) {
        *self = Mode::Follower(Follower);
        let follower = cast_unsafe!(self, Mode::Follower);
        follower.on_follower(context);
    }

    fn on_candidate<E: ServerEgress>(&mut self, context: &mut Context, io_egress: &mut E) {
        *self = Mode::Candidate(Candidate::default());
        let candidate = cast_unsafe!(self, Mode::Candidate);

        match candidate.on_candidate(context, io_egress) {
            ModeTransition::None => (),
            ModeTransition::ToLeader => {
                // If the quorum size is 1, then a candidate will become leader immediately
                self.on_leader();
            }
            ModeTransition::ToCandidate | ModeTransition::ToFollower => {
                unreachable!("Invalid mode transition");
            }
        }
    }

    fn on_leader(&mut self) {
        *self = Mode::Leader(Leader);
        let leader = cast_unsafe!(self, Mode::Leader);
        leader.on_leader();
    }

    fn quorum(context: &Context) -> usize {
        let peer_plus_self = context.peer_map.len() + 1;
        let half = peer_plus_self / 2;
        half + 1
    }
}

#[must_use]
pub enum ModeTransition {
    None,
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
        peer::PeerInfo,
        raft_state::RaftState,
        server::ServerId,
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;

    #[tokio::test]
    async fn test_quorum() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let mut peer_map = PeerInfo::mock_as_map(&[]);
        let mut state = RaftState::new(timeout, &peer_map);
        let server_id = ServerId::new([6; 16]);
        let mut context = Context {
            server_id,
            raft_state: &mut state,
            peer_map: &mut peer_map,
        };
        assert_eq!(Mode::quorum(&context), 1);

        let mut peer_map = PeerInfo::mock_as_map(&[1]);
        context.peer_map = &mut peer_map;
        assert_eq!(Mode::quorum(&context), 2);

        let mut peer_map = PeerInfo::mock_as_map(&[1, 2]);
        context.peer_map = &mut peer_map;
        assert_eq!(Mode::quorum(&context), 2);

        let mut peer_map = PeerInfo::mock_as_map(&[1, 2, 3]);
        context.peer_map = &mut peer_map;
        assert_eq!(Mode::quorum(&context), 3);
    }

    #[tokio::test]
    // recv append_entries with gt or eq term
    async fn candidate_stays_candidate_on_recv_append_entries() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let leader_id_fill = 2;
        let leader_id = ServerId::new([leader_id_fill; 16]);
        let mut peer_map = PeerInfo::mock_as_map(&[leader_id_fill]);
        let mut state = RaftState::new(timeout, &peer_map);
        let current_term = Term::from(2);
        state.current_term = current_term;

        let mut context = Context {
            server_id: ServerId::new([1; 16]),
            raft_state: &mut state,
            peer_map: &mut peer_map,
        };
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
        mode.on_recv(leader_id, append_entries, &mut context, &mut leader_io);

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

        let peer_fill = 2;
        let peer_id = ServerId::new([peer_fill; 16]);
        let mut peer_map = PeerInfo::mock_as_map(&[peer_fill]);
        let mut state = RaftState::new(timeout, &peer_map);
        state.current_term = current_term;

        let mut io = MockIo::new();
        let mut context = Context {
            server_id: ServerId::new([1; 16]),
            raft_state: &mut state,
            peer_map: &mut peer_map,
        };
        let mut mode = Mode::Candidate(Candidate::default());

        // Mock send AppendEntries to Candidate with `term => current_term`
        let append_entries = Rpc::new_append_entry(
            Term::from(1),
            peer_id,
            TermIdx::initial(),
            Idx::initial(),
            vec![],
        );
        mode.on_recv(peer_id, append_entries, &mut context, &mut io);

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
