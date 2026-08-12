//! A deterministic simulation of a Raft cluster.
//!
//! Unit tests inside raft-rs drive one Mode in isolation against a MockIo. This drives N Servers
//! wired to each other through the real byte queues, which is the only thing that proves Raft
//! works end to end.

pub mod cluster;
mod fuzz;

#[cfg(test)]
mod election;
#[cfg(test)]
mod large_cluster;
#[cfg(test)]
mod replication;
#[cfg(test)]
mod router;
