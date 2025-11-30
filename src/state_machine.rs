use crate::{
    log::{Entry, Idx},
    server::PeerId,
};

#[derive(Debug)]
pub struct StateMachine {
    entries: Vec<CommitEntry>,
}

impl StateMachine {
    pub fn new() -> Self {
        StateMachine { entries: vec![] }
    }

    pub fn apply(&mut self, data: CommitEntry) {
        self.entries.push(data);
    }
}

#[derive(Debug)]
pub struct CommitEntry {
    // The Entry matching the Log entry.
    pub(crate) entry: Entry,

    // The lastApplied log Idx for this Entry.
    pub(crate) log_last_applied_idx: Idx,

    // The PeerId that initiated (on_recv) this commit.
    pub(crate) peer_id: PeerId,

    // The current mode when this Entry was comitted.
    pub(crate) mode: CurrentMode,
}

#[derive(Debug, Copy, Clone)]
pub enum CurrentMode {
    Follower,
    Leader,
}
