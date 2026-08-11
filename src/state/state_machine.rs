use crate::{
    server::PeerId,
    state::{entry::Entry, log::Idx},
};

/// Entries/data that a majority of Raft servers agree on. This is permanent storage which can be
/// queried by the Application to figure out what data has been 'committed'.
///
/// The Log is meant for use by the Raft protocol (data that might not be fully replicated yet).
/// The StateMachine is meant for use by the Application (data that is replicated on majority of
/// servers).
///
/// 'commit' vs 'apply'
/// - The leader 'commits' entries to the state machine.
/// - The follower 'applies' entries to the state machine.
///
//% Compliance:
//% **Safety:** (State Machine Safety Property). If a server has applied a log entry to state
//% machine, then no other server will apply a different entry to the same log index
#[derive(Debug)]
pub struct StateMachine {
    entries: Vec<CommitEntry>,
}

impl StateMachine {
    pub fn new() -> Self {
        StateMachine { entries: vec![] }
    }

    // Commit entry to the StateMachine.
    //
    // 'commit' vs 'apply'
    // - The leader 'commits' entries to the state machine.
    // - The follower 'applies' entries to the state machine.
    pub fn commit_entry(&mut self, data: CommitEntry) {
        self.entries.push(data);
    }
}

/// Data which is committed to the StateMachine.
#[derive(Debug)]
pub struct CommitEntry {
    // The Entry matching the Log entry.
    pub(crate) entry: Entry,

    // The lastApplied log Idx for this Entry.
    pub(crate) log_last_applied_idx: Idx,

    // The PeerId that initiated (on_recv) this commit.
    pub(crate) updated_peers: Vec<PeerId>,

    // The current mode when this Entry was comitted.
    pub(crate) mode: CurrentMode,
}

#[derive(Debug, Copy, Clone)]
pub enum CurrentMode {
    Follower,
    Leader,
}
