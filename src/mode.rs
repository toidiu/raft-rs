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
    mode::{candidate::CandidateState, follower::FollowerState, leader::LeaderState},
    rpc::Rpc,
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

impl Mode {
    fn on_convert_to_follower<T: ServerTx>(&mut self, tx: &mut T) {
        *self = Mode::Follower(FollowerState);
        self.on_convert(tx);
    }

    fn on_convert_to_candidate<T: ServerTx>(&mut self, tx: &mut T) {
        *self = Mode::Candidate(CandidateState);
        self.on_convert(tx);
    }
}

impl Action for Mode {
    fn on_convert<T: ServerTx>(&mut self, tx: &mut T) {
        match self {
            Mode::Follower(follower) => follower.on_convert(tx),
            Mode::Candidate(candidate) => candidate.on_convert(tx),
            Mode::Leader(leader) => leader.on_convert(tx),
        }
    }

    fn on_timeout<T: ServerTx>(&mut self, tx: &mut T) {
        match self {
            Mode::Follower(_follower) => {
                //% Compliance:
                //% If election timeout elapses without receiving AppendEntries RPC from current
                //% leader or granting vote to candidate: convert to candidate
                self.on_convert_to_candidate(tx);
            }
            Mode::Candidate(candidate) => candidate.on_timeout(tx),
            Mode::Leader(leader) => leader.on_timeout(tx),
        }
    }

    fn on_recv<T: ServerTx>(&mut self, tx: &mut T, rpc: Rpc, state: &mut State) {
        //% Compliance
        //% If RPC request or response contains term T > currentTerm: set currentTerm = T, convert
        //% to follower (§5.1)
        if rpc.term() > &state.current_term {
            state.current_term = *rpc.term();
            self.on_convert_to_follower(tx);
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
}

trait Action {
    fn on_convert<T: ServerTx>(&mut self, tx: &mut T);

    fn on_timeout<T: ServerTx>(&mut self, tx: &mut T);

    fn on_recv<T: ServerTx>(&mut self, tx: &mut T, rpc: Rpc, state: &mut State);
}
