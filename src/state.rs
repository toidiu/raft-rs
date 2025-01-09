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
    pub fn new(election_timer: Timeout, peer_map: &BTreeMap<ServerId, Peer>) -> Self {
        let log = Log::new();
        let mut next_idx_map = BTreeMap::new();
        let mut match_idx_map = BTreeMap::new();

        //% Compliance:
        //% `nextIndex[]` for each server, index of the next log entry to send to that server
        //% (initialized to leader last log index + 1)
        let next_log_idx = log.next_idx();

        //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server
        //% (initialized to 0, increases monotonically)
        let match_idx = Idx::initial();

        for (id, peer) in peer_map.iter() {
            let Peer { id: _, io: _ } = peer;

            next_idx_map.insert(*id, next_log_idx);
            match_idx_map.insert(*id, match_idx);
        }
        State {
            current_term: Term::initial(),
            voted_for: None,
            log,
            commit_idx: Idx::initial(),
            last_applied: Idx::initial(),
            next_idx: next_idx_map,
            match_idx: match_idx_map,
            election_timer,
        }
    }
}
