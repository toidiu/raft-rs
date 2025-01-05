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

mod candidate;
mod follower;

use crate::{
    io::ServerTx,
    log::{Idx, Log, Term},
    server::ServerId,
    state::{candidate::CandidateState, follower::FollowerState},
};
use std::collections::BTreeMap;

struct State {
    //  ==== Persistent state on all servers ====
    //% Compliance
    //% `currentTerm` latest term server has seen (initialized to 0 on first boot, increases monotonically)
    pub current_term: Term,

    //% Compliance
    //% `votedFor` `candidateId` that received vote in current term (or null if none)
    voted_for: Option<ServerId>,

    //% Compliance
    //% `log[]` log entries; each entry contains command for state machine, and term when entry was received by leader (first index is 1)
    log: Log,

    // ==== Volatile state on all servers ====
    //% Compliance
    //% `commitIndex` index of highest log entry known to be committed (initialized to 0, increases monotonically)
    commit_idx: Idx,

    //% Compliance
    //% lastApplied: index of highest log entry applied to state machine (initialized to 0, increases monotonically)
    last_applied: Idx,

    // ==== Volatile state on leaders ====
    //% Compliance
    //% `nextIndex[]` for each server, index of the next log entry to send to that server (initialized to leader last log index + 1)
    next_idx: BTreeMap<ServerId, Idx>,

    //% Compliance
    //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server (initialized to 0, increases monotonically)
    match_idx: BTreeMap<ServerId, Idx>,

    // === Other ===
    mode: Mode,
}

enum Mode {
    Follower(FollowerState),
    Candidate(CandidateState),
    Leader,
}

trait Action {
    fn on_convert<T: ServerTx>(&mut self, io: &mut T);

    fn on_timeout<T: ServerTx>(&mut self, io: &mut T);
}

impl Action for Mode {
    fn on_convert<T: ServerTx>(&mut self, io: &mut T) {
        match self {
            Mode::Follower(follower) => follower.on_convert(io),
            Mode::Candidate(candidate) => candidate.on_convert(io),
            Mode::Leader => todo!(),
        }
    }

    fn on_timeout<T: ServerTx>(&mut self, io: &mut T) {
        match self {
            Mode::Follower(_follower) => {
                //% Compliance:
                //% If election timeout elapses without receiving AppendEntries RPC from current leader or granting vote to candidate: convert to candidate
                *self = Mode::Candidate(CandidateState);
                self.on_convert(io);
            }
            Mode::Candidate(candidate) => candidate.on_timeout(io),
            Mode::Leader => todo!(),
        }
    }
}

impl State {
    pub fn new() -> Self {
        State {
            current_term: Term::initial(),
            voted_for: None,
            log: Log::new(),
            commit_idx: Idx::initial(),
            last_applied: Idx::initial(),
            next_idx: BTreeMap::new(),
            match_idx: BTreeMap::new(),
            mode: Mode::Follower(FollowerState),
        }
    }

    pub fn on_timeout<T: ServerTx>(&mut self, io: &mut T) {
        self.mode.on_timeout(io);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_state() {
        let s = State::new();
        assert!(matches!(s.mode, Mode::Follower(_)));
    }
}
