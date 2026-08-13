//! Raft safety properties, asserted against the whole cluster.
//!
//! These are the oracle. A test drives the cluster and checks what it set out to check, while
//! these hold after any sequence of operations at all, which is what makes them worth running from
//! the fuzzer on inputs nobody wrote by hand.

use crate::cluster::{node::ServerIdx, Cluster};
use raft_rs::state::entry::Entry;

impl Cluster {
    /// How many entries this server considers committed.
    fn committed_len(&self, idx: ServerIdx) -> usize {
        let commit_idx = self.commit_idx(idx);

        if commit_idx.is_initial() {
            0
        } else {
            // Idx counts from 1, so the position of the last committed entry is also the count.
            commit_idx.as_log_idx() + 1
        }
    }

    //% Compliance:
    //% Log Matching: if two logs contain an entry with the same index and term, then the logs are
    //% identical in all entries up through the given index (§5.3)
    ///
    /// Matching terms are the whole property. Two servers holding different entries at the same
    /// index is ordinary Raft whenever the terms differ, and is exactly what a Leader's `next_idx`
    /// walkback repairs. Asserting that no index ever disagrees would fire on any partitioned
    /// ex-Leader that appended before it lost contact, which is a legal execution.
    ///
    /// This says nothing about whether the disagreeing entries were committed. See
    /// [`Self::assert_committed_entries_agree`] for that.
    pub fn assert_logs_match(&self) {
        let logs: Vec<Vec<Entry>> = self.idxs().map(|idx| self.log_entries(idx)).collect();

        for (position, server) in self.idxs().enumerate() {
            // Check every pair of servers once. Starting past `server` avoids comparing a pair
            // against itself, and avoids re-checking it with the two sides swapped.
            for other_server in self.idxs().skip(position + 1) {
                let server_log = &logs[server.0];
                let other_log = &logs[other_server.0];

                // The shorter log is a server that is merely behind, which is legal.
                let shared_len = server_log.len().min(other_log.len());

                for position in 0..shared_len {
                    // Different terms at one index mean the entries came from different Leaders.
                    // Raft promises nothing about them until one side is overwritten.
                    if server_log[position].term != other_log[position].term {
                        continue;
                    }

                    assert_eq!(
                        server_log[..=position],
                        other_log[..=position],
                        "server {server} and server {other_server} agree on the term at index {} \
                         but disagree somewhere before it",
                        position + 1
                    );
                }
            }
        }
    }

    //% Compliance:
    //% State Machine Safety: if a server has applied a log entry at a given index to its state
    //% machine, no other server will ever apply a different log entry for the same index (§5.4.3)
    ///
    /// Compares the prefix both servers call committed, then the commands they actually applied.
    ///
    /// [`Self::assert_logs_match`] deliberately permits two servers to hold different uncommitted
    /// entries at one index. Without this check the oracle would also accept a Leader overwriting
    /// an entry a quorum had already agreed on, which is the one failure Raft exists to prevent.
    pub fn assert_committed_entries_agree(&self) {
        let logs: Vec<Vec<Entry>> = self.idxs().map(|idx| self.log_entries(idx)).collect();

        for (position, server) in self.idxs().enumerate() {
            for other_server in self.idxs().skip(position + 1) {
                // Only the entries both servers have committed are constrained. A server that has
                // committed less is merely behind.
                let shared_len = self
                    .committed_len(server)
                    .min(self.committed_len(other_server));

                assert_eq!(
                    logs[server.0][..shared_len],
                    logs[other_server.0][..shared_len],
                    "committed entries diverge between server {server} and server {other_server}"
                );

                // Applying is committing plus executing, so the same prefix rule holds for the
                // commands the state machines actually ran, in the order they ran them.
                let server_applied = self.applied_commands(server);
                let other_applied = self.applied_commands(other_server);
                let shared_len = server_applied.len().min(other_applied.len());

                assert_eq!(
                    server_applied[..shared_len],
                    other_applied[..shared_len],
                    "applied commands diverge between server {server} and server {other_server}"
                );
            }
        }
    }
}
