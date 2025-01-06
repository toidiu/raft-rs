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
    io::ServerTx,
    log::Log,
    macros::cast_unsafe,
    mode::{candidate::CandidateState, follower::FollowerState, leader::LeaderState},
    rpc::Rpc,
    server::ServerId,
    state::State,
};

mod candidate;
mod follower;
mod leader;

pub enum Mode {
    Follower(FollowerState),
    Candidate(CandidateState),
    Leader(LeaderState),
}

pub struct Context<'a> {
    server_id: ServerId,
    state: &'a mut State,
    log: &'a Log,
    peer_list: &'a Vec<ServerId>,
}

impl Mode {
    fn on_timeout<T: ServerTx>(&mut self, tx: &mut T, context: &mut Context) {
        match self {
            Mode::Follower(_follower) => {
                //% Compliance:
                //% If election timeout elapses without receiving AppendEntries RPC from current
                //% leader or granting vote to candidate: convert to candidate
                self.on_candidate(tx, context);
            }
            Mode::Candidate(candidate) => candidate.on_timeout(tx, context),
            Mode::Leader(leader) => leader.on_timeout(tx),
        }
    }

    fn on_recv<T: ServerTx>(&mut self, tx: &mut T, rpc: Rpc, state: &mut State) {
        //% Compliance:
        //% If RPC request or response contains term T > currentTerm: set currentTerm = T, convert
        //% to follower (§5.1)
        if rpc.term() > &state.current_term {
            state.current_term = *rpc.term();
            self.on_follower(tx);
        }

        match self {
            Mode::Follower(follower) => {
                //% Compliance:
                //% If election timeout elapses without receiving AppendEntries RPC from current
                //% leader or granting vote to candidate: convert to candidate
                follower.on_recv(tx, rpc, state);
            }
            Mode::Candidate(candidate) => candidate.on_recv(tx, rpc, state),
            Mode::Leader(leader) => leader.on_recv(tx, rpc, state),
        }
    }

    fn on_follower<T: ServerTx>(&mut self, tx: &mut T) {
        *self = Mode::Follower(FollowerState);
        let follower = cast_unsafe!(self, Mode::Follower);
        follower.on_follower(tx);
    }

    fn on_candidate<T: ServerTx>(&mut self, tx: &mut T, context: &mut Context) {
        *self = Mode::Candidate(CandidateState);
        let candidate = cast_unsafe!(self, Mode::Candidate);
        candidate.on_candidate(tx, context);
    }

    fn on_leader<T: ServerTx>(&mut self, tx: &mut T) {
        *self = Mode::Leader(LeaderState);
        let leader = cast_unsafe!(self, Mode::Leader);
        leader.on_leader(tx);
    }
}
