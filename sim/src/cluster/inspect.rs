//! Read-only views into the cluster, for test assertions.

use crate::cluster::{node::ServerIdx, Cluster};
use raft_rs::{
    server::{ClientResponse, PeerId},
    state::{
        entry::Entry,
        log::{Idx, Term},
    },
};

impl Cluster {
    /// Packets that have left a sender and not yet arrived.
    pub fn in_flight_count(&self) -> usize {
        self.in_flight_packets.len()
    }

    /// Every server position.
    pub fn idxs(&self) -> impl Iterator<Item = ServerIdx> {
        (0..self.nodes.len()).map(ServerIdx)
    }

    /// The current Leader, if exactly one running server thinks it leads.
    ///
    /// None while an election is in flight, and also when two servers each believe they lead —
    /// which is legal during a term change and is not by itself a safety violation.
    pub fn leader(&self) -> Option<ServerIdx> {
        let mut found = None;

        for idx in self.idxs() {
            if self.has_crashed(idx) || !self.is_leader(idx) {
                continue;
            }

            if found.is_some() {
                // More than one server claims to lead. Report no Leader rather than picking one.
                return None;
            }
            found = Some(idx);
        }

        found
    }

    /// Does this server believe it is the Leader. Two can at once, mid term change.
    pub fn is_leader(&self, idx: ServerIdx) -> bool {
        self.node(idx).server.mode.is_leader()
    }

    /// Is this server a Follower, as opposed to a Candidate or Leader.
    pub fn is_follower(&self, idx: ServerIdx) -> bool {
        self.node(idx).server.mode.is_follower()
    }

    /// Has a test stopped this server.
    pub fn has_crashed(&self, idx: ServerIdx) -> bool {
        self.node(idx).crashed
    }

    /// The latest term this server has seen. Servers agree on it once a Leader settles.
    pub fn current_term(&self, idx: ServerIdx) -> Term {
        self.node(idx).server.state.current_term
    }

    /// How far this server considers the log committed. Followers trail the Leader by a round.
    pub fn commit_idx(&self, idx: ServerIdx) -> Idx {
        *self.node(idx).server.state.commit_idx()
    }

    /// The whole log, replicated but not necessarily committed.
    pub fn log_entries(&self, idx: ServerIdx) -> Vec<Entry> {
        let log = &self.node(idx).server.state.log;
        (1..=log.test_len() as u64)
            .map(|log_idx| log.test_get_unchecked(log_idx))
            .collect()
    }

    /// The Idx of every Entry applied to the StateMachine, in apply order.
    pub fn applied_idxs(&self, idx: ServerIdx) -> Vec<Idx> {
        self.node(idx).server.query_state_machine()
    }

    /// The PeerId other servers address this one by.
    pub fn as_peer_id(&self, idx: ServerIdx) -> PeerId {
        PeerId::new(*self.node(idx).server.server_id.as_bytes())
    }

    /// Who this server would redirect a client to. None unless it is a Follower that has accepted
    /// an AppendEntries.
    pub fn known_leader(&self, idx: ServerIdx) -> Option<PeerId> {
        let server = &self.node(idx).server;
        server.mode.current_leader(&server.server_id)
    }

    /// Submit a client command.
    pub fn client_request(&mut self, idx: ServerIdx, command: u8) -> ClientResponse {
        self.node_mut(idx).server.on_client_request(command)
    }

    /// Log Matching Property (§5.3): if two logs hold an entry at the same index, the entries are
    /// identical.
    ///
    /// Compares the prefix both servers hold, so a Follower that is merely behind passes; only an
    /// actual conflict at a shared index fails. Worth asserting after any test that replicates,
    /// since it catches divergence the test's own assertions would not look for.
    pub fn assert_logs_match(&self) {
        for (position, server) in self.idxs().enumerate() {
            // Check every pair of servers once. Starting past `server` avoids comparing a pair
            // against itself, and avoids re-checking it with the two sides swapped.
            for other_server in self.idxs().skip(position + 1) {
                let server_log = self.log_entries(server);
                let other_log = self.log_entries(other_server);

                // The shorter log is a server that is merely behind, which is legal. Only the
                // indexes both of them hold can conflict.
                let shared_len = server_log.len().min(other_log.len());

                assert_eq!(
                    server_log[..shared_len],
                    other_log[..shared_len],
                    "logs diverge between server {server} and server {other_server}"
                );
            }
        }
    }
}
