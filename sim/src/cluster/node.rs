use raft_rs::{
    queue::NetworkQueueImpl,
    server::{Id, Server},
};
use std::fmt;

/// Which server in the Cluster, by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServerIdx(pub(super) usize);

impl fmt::Display for ServerIdx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A Raft server process for Integration testing.
pub struct Node {
    pub(super) server: Server,

    // Server's IO queue.
    pub(super) queue: NetworkQueueImpl,

    // A crashed Node is one that stopped running:
    // - sends nothing
    // - receives nothing
    // - election timer never fires
    pub(super) crashed: bool,
}

impl Node {
    /// The Id peers address this server by.
    pub(super) fn id(&self) -> Id {
        self.server.server_id.into_id()
    }

    pub(super) fn has_crashed(&self) -> bool {
        self.crashed
    }
}
