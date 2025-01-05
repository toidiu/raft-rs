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
    log::{Idx, Log, Term},
    server::ServerId,
};

pub struct State {
    //  ==== Persistent state on all servers ====
    //% Compliance
    //% `currentTerm` latest term server has seen (initialized to 0 on first boot, increases
    //% monotonically)
    pub current_term: Term,

    //% Compliance
    //% `votedFor` `candidateId` that received vote in current term (or null if none)
    voted_for: Option<ServerId>,

    //% Compliance
    //% `log[]` log entries; each entry contains command for state machine, and term when entry was
    //% received by leader (first index is 1)
    log: Log,

    // ==== Volatile state on all servers ====
    //% Compliance
    //% `commitIndex` index of highest log entry known to be committed (initialized to 0, increases
    //% monotonically)
    commit_idx: Idx,

    //% Compliance
    //% lastApplied: index of highest log entry applied to state machine (initialized to 0,
    //% increases monotonically)
    last_applied: Idx,
}

impl State {
    pub fn new() -> Self {
        State {
            current_term: Term::initial(),
            voted_for: None,
            log: Log::new(),
            commit_idx: Idx::initial(),
            last_applied: Idx::initial(),
        }
    }
}
