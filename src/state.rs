use crate::{
    log::{Idx, Log, Term},
    peer::Peer,
    server::ServerId,
    timeout::Timeout,
};
use std::collections::BTreeMap;

pub struct State {
    //  ==== Persistent state on all servers ====
    //% Compliance:
    //% `currentTerm` latest term server has seen (initialized to 0 on first boot, increases
    //% monotonically)
    pub current_term: Term,

    //% Compliance:
    //% `votedFor` `candidateId` that received vote in current term (or null if none)
    pub voted_for: Option<ServerId>,

    //% Compliance:
    //% `log[]` log entries; each entry contains command for state machine, and term when entry was
    //% received by leader (first index is 1)
    pub log: Log,

    // ==== Volatile state on all servers ====
    //% Compliance:
    //% `commitIndex` index of highest log entry known to be committed (initialized to 0, increases
    //% monotonically)
    pub commit_idx: Idx,

    //% Compliance:
    //% lastApplied: index of highest log entry applied to state machine (initialized to 0,
    //% increases monotonically)
    pub last_applied: Idx,

    // ==== Volatile state on leaders ====
    //% Compliance:
    //% `nextIndex[]` for each server, index of the next log entry to send to that server
    //% (initialized to leader last log index + 1)
    pub next_idx: BTreeMap<ServerId, Idx>,

    //% Compliance:
    //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server
    //% (initialized to 0, increases monotonically)
    pub match_idx: BTreeMap<ServerId, Idx>,

    pub election_timer: Timeout,
}

impl State {
    pub fn new(election_timer: Timeout, peer_list: &[Peer]) -> Self {
        let mut next_idx = BTreeMap::new();
        let mut match_idx = BTreeMap::new();

        //% Compliance:
        //% `nextIndex[]` for each server, index of the next log entry to send to that server
        //% (initialized to leader last log index + 1)
        let last_log_index_plus_1 = Idx::from(1);

        for peer in peer_list.iter() {
            let Peer { id, io: _ } = peer;

            next_idx.insert(*id, last_log_index_plus_1);
            match_idx.insert(*id, last_log_index_plus_1);
        }
        State {
            current_term: Term::initial(),
            voted_for: None,
            log: Log::new(),
            commit_idx: Idx::initial(),
            last_applied: Idx::initial(),
            next_idx,
            match_idx,
            election_timer,
        }
    }
}
