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

    /// How many servers the cluster was built with.
    pub fn server_count(&self) -> usize {
        self.nodes.len()
    }

    /// The current Leader, if exactly one running server thinks it leads.
    ///
    /// None while an election is in flight, and also when two servers each believe they lead.
    /// That is legal during a term change and is not by itself a safety violation.
    pub fn leader(&self) -> Option<ServerIdx> {
        let mut found = None;

        for idx in self.idxs() {
            if self.is_paused(idx) || !self.is_leader(idx) {
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
    pub fn is_paused(&self, idx: ServerIdx) -> bool {
        self.node(idx).paused
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
    pub fn query_state_machine(&self, idx: ServerIdx) -> Vec<Idx> {
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

    /// The commands the StateMachine applied, in apply order.
    pub fn applied_commands(&self, idx: ServerIdx) -> Vec<u8> {
        self.node(idx).server.state.applied_commands()
    }

    /// Submit a client command. None when the server is paused and never saw the request.
    ///
    /// A paused server is a process that is not running, so it cannot answer a client any more
    /// than it can answer a peer. Letting one accept a command would let a Leader the cluster has
    /// already moved past keep appending to its log, which manufactures log divergence that no
    /// reachable Raft execution produces.
    pub fn client_request(&mut self, idx: ServerIdx, command: u8) -> Option<ClientResponse> {
        if self.is_paused(idx) {
            return None;
        }

        Some(self.node_mut(idx).server.on_client_request(command))
    }
}
