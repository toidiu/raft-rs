//! Things that go wrong, injected by a test.
//!
//! Today only whole-server failure. Network partitions and per-packet drops belong here too, and
//! land in [`super::router`] when they arrive.

use crate::tests::cluster::{node::ServerIdx, Cluster};

impl Cluster {
    /// Stop a server.
    ///
    /// The rest of the cluster stops hearing from it, which is all Raft needs to start an
    /// election. Crashing enough servers to break quorum is how a test shows nothing commits.
    pub fn crash(&mut self, idx: ServerIdx) {
        self.node_mut(idx).crashed = true;
    }

    /// Restart a crashed server.
    ///
    /// It comes back holding exactly the state it had when it stopped, and campaigns straight away
    /// since its election timeout expired long ago. If the cluster elected a new Leader while it
    /// was down, that Leader's higher term demotes it and then repairs its log.
    pub fn restart(&mut self, idx: ServerIdx) {
        self.node_mut(idx).crashed = false;
    }
}
