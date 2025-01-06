use crate::{
    log::{Idx, Log, Term},
    server::ServerId,
};
use std::collections::BTreeMap;

pub struct State {
    //  ==== Persistent state on all servers ====
    //% Compliance
    //% `currentTerm` latest term server has seen (initialized to 0 on first boot, increases
    //% monotonically)
    pub current_term: Term,

    //% Compliance
    //% `votedFor` `candidateId` that received vote in current term (or null if none)
    pub voted_for: Option<ServerId>,

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

    // ==== Volatile state on leaders ====
    //% Compliance
    //% `nextIndex[]` for each server, index of the next log entry to send to that server
    //% (initialized to leader last log index + 1)
    next_idx: BTreeMap<ServerId, Idx>,

    //% Compliance
    //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server
    //% (initialized to 0, increases monotonically)
    match_idx: BTreeMap<ServerId, Idx>,
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
        }
    }
}
