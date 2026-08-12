//! Things that go wrong, injected by a test.
//!
//! Today only whole-server failure. Network partitions and per-packet drops belong here too, and
//! land in [`super::router`] when they arrive.

use crate::cluster::{node::ServerIdx, Cluster};

impl Cluster {
    /// Stop a server without losing its state.
    ///
    /// The rest of the cluster stops hearing from it, which is all Raft needs to start an
    /// election. Pausing enough servers to break quorum is how a test shows nothing commits.
    ///
    /// This is not a crash. Everything the server had in memory is still there when it resumes. A
    /// true crash that loses unpersisted state is a separate fault and does not exist yet.
    pub fn pause(&mut self, idx: ServerIdx) {
        self.node_mut(idx).paused = true;
    }

    /// Start a paused server again.
    ///
    /// It comes back holding exactly the state it had when it stopped, and campaigns straight away
    /// since its election timeout expired long ago. If the cluster elected a new Leader while it
    /// was down, that Leader's higher term demotes it and then repairs its log.
    pub fn resume(&mut self, idx: ServerIdx) {
        self.node_mut(idx).paused = false;
    }
}
