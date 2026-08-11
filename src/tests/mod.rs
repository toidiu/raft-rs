//! Integration tests: a cluster of Servers exchanging real encoded Packets.
//!
//! Unit tests drive one Mode in isolation against a MockIo. These drive N Servers wired to each
//! other through the byte queues, which is the only thing that proves Raft works end to end.

mod cluster;

// Tests
mod election;
mod router;
