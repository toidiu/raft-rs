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
    io::ServerIO,
    macros::cast_unsafe,
    mode::{candidate::CandidateState, follower::FollowerState, leader::LeaderState},
    rpc::Rpc,
    server::Context,
};

mod candidate;
mod follower;
mod leader;

pub enum Mode {
    Follower(FollowerState),
    Candidate(CandidateState),
    Leader(LeaderState),
}

impl Mode {
    fn on_timeout<IO: ServerIO>(&mut self, tx: &mut IO, context: &mut Context<IO>) {
        match self {
            Mode::Follower(_follower) => {
                //% Compliance:
                //% If election timeout elapses without receiving AppendEntries RPC from current
                //% leader or granting vote to candidate: convert to candidate
                self.on_candidate(tx, context);
            }
            Mode::Candidate(candidate) => {
                let transition = candidate.on_timeout(context);
                self.handle_mode_transition(tx, transition, context);
            }

            Mode::Leader(leader) => leader.on_timeout(tx),
        }
    }

    fn on_recv<IO: ServerIO>(&mut self, tx: &mut IO, rpc: Rpc, context: &mut Context<IO>) {
        //% Compliance:
        //% If RPC request or response contains term T > currentTerm: set currentTerm = T, convert
        //% to follower (§5.1)
        if rpc.term() > &context.state.current_term {
            context.state.current_term = *rpc.term();
            self.on_follower(tx);
        }

        let rpc = match self {
            Mode::Follower(follower) => {
                //% Compliance:
                //% If election timeout elapses without receiving AppendEntries RPC from current
                //% leader or granting vote to candidate: convert to candidate
                follower.on_recv(tx, rpc, context);
                None
            }
            Mode::Candidate(candidate) => {
                let (transition, rpc) = candidate.on_recv(tx, rpc, context);
                self.handle_mode_transition(tx, transition, context);

                //% Compliance:
                //% another server establishes itself as a leader
                //
                //% Compliance:
                //% recognize the server as the new leader
                // If converted
                rpc
            }
            Mode::Leader(leader) => {
                leader.on_recv(rpc, context);
                None
            }
        };

        // Attempt to process the RPC again.
        //
        // An RPC might only be partially processed if it results in a ModeTransition and should be
        // processed again by the new Mode.
        if let Some(rpc) = rpc {
            self.on_recv(tx, rpc, context)
        }
    }

    fn handle_mode_transition<IO: ServerIO>(
        &mut self,
        tx: &mut IO,
        transition: ModeTransition,
        context: &mut Context<IO>,
    ) {
        match transition {
            ModeTransition::None => (),
            ModeTransition::ToFollower => self.on_follower(tx),
            ModeTransition::ToCandidate => self.on_candidate(tx, context),
            ModeTransition::ToLeader => self.on_leader(tx),
        }
    }

    fn on_follower<IO: ServerIO>(&mut self, tx: &mut IO) {
        *self = Mode::Follower(FollowerState);
        let follower = cast_unsafe!(self, Mode::Follower);
        follower.on_follower(tx);
    }

    fn on_candidate<IO: ServerIO>(&mut self, tx: &mut IO, context: &mut Context<IO>) {
        *self = Mode::Candidate(CandidateState::default());
        let candidate = cast_unsafe!(self, Mode::Candidate);

        match candidate.on_candidate(context) {
            ModeTransition::None => (),
            ModeTransition::ToLeader => {
                // If the quorum size is 1, then a candidate will become leader immediately
                self.on_leader(tx);
            }
            ModeTransition::ToCandidate | ModeTransition::ToFollower => {
                unreachable!("Invalid mode transition");
            }
        }
    }

    fn on_leader<IO: ServerIO>(&mut self, tx: &mut IO) {
        *self = Mode::Leader(LeaderState);
        let leader = cast_unsafe!(self, Mode::Leader);
        leader.on_leader(tx);
    }

    fn quorum<IO: ServerIO>(context: &Context<IO>) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        io::testing::MockIO,
        log::{Idx, Term, TermIdx},
        peer::Peer,
        server::ServerId,
        state::State,
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;
    use s2n_codec::DecoderBuffer;

    #[tokio::test]
    async fn test_quorum() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let mut peer_map = Peer::mock_as_map(&[]);
        let mut state = State::new(timeout, &peer_map);
        let server_id = ServerId::new([6; 16]);
        let mut context = Context {
            server_id,
            state: &mut state,
            peer_map: &mut peer_map,
        };
        assert_eq!(Mode::quorum(&context), 1);

        let mut peer_map = Peer::mock_as_map(&[1]);
        context.peer_map = &mut peer_map;
        assert_eq!(Mode::quorum(&context), 2);

        let mut peer_map = Peer::mock_as_map(&[1, 2]);
        context.peer_map = &mut peer_map;
        assert_eq!(Mode::quorum(&context), 2);

        let mut peer_map = Peer::mock_as_map(&[1, 2, 3]);
        context.peer_map = &mut peer_map;
        assert_eq!(Mode::quorum(&context), 3);
    }

    #[tokio::test]
    async fn candidate_recv_append_entries_with_gt_eq_term() {
        let current_term = Term::from(2);
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let peer_fill = 2;
        let peer_id = ServerId::new([peer_fill; 16]);
        let mut peer_map = Peer::mock_as_map(&[peer_fill]);
        let mut state = State::new(timeout, &peer_map);
        state.current_term = current_term;

        let mut context = Context {
            server_id: ServerId::new([1; 16]),
            state: &mut state,
            peer_map: &mut peer_map,
        };
        let mut mode = Mode::Candidate(CandidateState::default());
        let mut io = MockIO::new();

        // Mock send AppendEntries to Candidate with `term => current_term`
        let append_entries = Rpc::new_append_entry(
            current_term,
            peer_id,
            TermIdx::initial(),
            Idx::initial(),
            vec![],
        );
        mode.on_recv(&mut io, append_entries, &mut context);

        // expect Mode::Follower
        assert!(matches!(mode, Mode::Follower(_)));

        // expect Follower to send RespAppendEntries acknowledging the leader
        // construct RPC to compare
        let expected_rpc = Rpc::new_append_entry_resp(current_term, true);
        let rpc_bytes = io.send_queue.pop().unwrap();
        assert!(io.send_queue.is_empty());
        let buffer = DecoderBuffer::new(&rpc_bytes);
        let (sent_request_vote, _) = buffer.decode::<Rpc>().unwrap();
        assert_eq!(expected_rpc, sent_request_vote);
    }

    #[tokio::test]
    async fn candidate_recv_append_entries_with_smaller_term() {
        let current_term = Term::from(2);

        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let peer_fill = 2;
        let peer_id = ServerId::new([peer_fill; 16]);
        let mut peer_map = Peer::mock_as_map(&[peer_fill]);
        let mut state = State::new(timeout, &peer_map);
        state.current_term = current_term;

        let mut context = Context {
            server_id: ServerId::new([1; 16]),
            state: &mut state,
            peer_map: &mut peer_map,
        };
        let mut mode = Mode::Candidate(CandidateState::default());
        let mut io = MockIO::new();

        // Mock send AppendEntries to Candidate with `term => current_term`
        let append_entries = Rpc::new_append_entry(
            Term::from(1),
            peer_id,
            TermIdx::initial(),
            Idx::initial(),
            vec![],
        );
        mode.on_recv(&mut io, append_entries, &mut context);

        // expect Mode::Follower
        assert!(matches!(mode, Mode::Candidate(_)));

        // expect Follower to send RespAppendEntries acknowledging the leader
        // construct RPC to compare
        let expected_rpc = Rpc::new_append_entry_resp(current_term, false);
        let rpc_bytes = io.send_queue.pop().unwrap();
        assert!(io.send_queue.is_empty());
        let buffer = DecoderBuffer::new(&rpc_bytes);
        let (sent_request_vote, _) = buffer.decode::<Rpc>().unwrap();
        assert_eq!(expected_rpc, sent_request_vote);
    }
}
