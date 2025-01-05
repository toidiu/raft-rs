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

trait Action {
    fn on_convert<T: ServerTx>(&mut self, io: &mut T);

    fn on_timeout<T: ServerTx>(&mut self, io: &mut T);

    fn on_recv<T: ServerTx>(&mut self, io: &mut T, rpc: Rpc, state: &mut State) {}
}

impl Action for Mode {
    fn on_convert<T: ServerTx>(&mut self, io: &mut T) {
        match self {
            Mode::Follower(follower) => follower.on_convert(io),
            Mode::Candidate(candidate) => candidate.on_convert(io),
            Mode::Leader(leader) => leader.on_convert(io),
        }
    }

    fn on_timeout<T: ServerTx>(&mut self, io: &mut T) {
        match self {
            Mode::Follower(_follower) => {
                //% Compliance:
                //% If election timeout elapses without receiving AppendEntries RPC from current
                //% leader or granting vote to candidate: convert to candidate
                *self = Mode::Candidate(CandidateState);
                self.on_convert(io);
            }
            Mode::Candidate(candidate) => candidate.on_timeout(io),
            Mode::Leader(leader) => leader.on_timeout(io),
        }
    }

    fn on_recv<T: ServerTx>(&mut self, io: &mut T, rpc: Rpc, state: &mut State) {
        //% Compliance
        //% If RPC request or response contains term T > currentTerm: set currentTerm = T, convert
        //% to follower (§5.1)
        if rpc.term() > &state.current_term {
            state.current_term = *rpc.term();
            *self = Mode::Follower(FollowerState);
            self.on_convert(io);
        }
    }
}
