//! Drive the cluster with a generated sequence of operations and look for a broken invariant.
//!
//! The simulator is async only because the clock it runs on lives inside a tokio runtime. That is
//! confined to [`Operation::apply`] and the `block_on` around it, so an Operation stays plain data
//! that bolero can generate, shrink, and print.

use crate::cluster::{node::ServerIdx, Cluster};
use bolero::{
    generator::{TypeGenerator, ValueGenerator},
    Driver,
};
use std::time::Duration;

pub const CLUSTER_SIZE: usize = 5;

// How far one RunUntil can advance the simulated clock.
//
// Election timeouts are 150-300ms and a Leader heartbeats every 50ms, so every decision worth
// exploring happens inside a few hundred milliseconds. One second covers several election rounds
// while still leaving a third of the range short enough to land mid-election, which is where the
// interesting interleavings are. Raising it mostly buys running a settled cluster forward with
// nothing left to happen, and eventually runs into MAX_EVENTS.
pub const MAX_RUN_MILLIS: u64 = 1_000;

// The largest cluster the simulation will build. Raft clusters are odd sized, so this covers 3, 5
// and 7. Raising it costs nothing but a slightly wider generator.
pub const MAX_CLUSTER_SIZE: u8 = 7;

/// An Operation's choice of which server to act on.
///
/// Generation is bounded to [`MAX_CLUSTER_SIZE`] rather than a full `u8` so that every distinct
/// value is a distinct decision. Across the whole `u8` range fifty one different bytes all name
/// server 0, so most mutations of that byte change nothing the cluster can observe and shrinking
/// cannot reduce it toward a simpler server.
///
/// Resolution is separate from generation because the cluster size is not known when the input is
/// generated, and will not be the same for every run once cluster size varies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerPick(pub u8);

impl ServerPick {
    /// The server this names in a cluster of `size`.
    ///
    /// Folds rather than rejects. A pick past the end of a smaller cluster still names a server,
    /// so no generated operation is wasted on a cluster that happens to be short.
    fn resolve(self, size: usize) -> ServerIdx {
        ServerIdx(self.0 as usize % size)
    }
}

impl TypeGenerator for ServerPick {
    fn generate<D: Driver>(driver: &mut D) -> Option<Self> {
        (0..MAX_CLUSTER_SIZE).generate(driver).map(ServerPick)
    }

    fn mutate<D: Driver>(&mut self, driver: &mut D) -> Option<()> {
        // Mutate in place through the same bounded generator, so a mutation stays a small step
        // rather than a fresh random server.
        (0..MAX_CLUSTER_SIZE).mutate(driver, &mut self.0)
    }
}

/// How far one Operation advances the simulated clock, in milliseconds.
///
/// Milliseconds rather than a [`Duration`] because a generated Duration spans nanoseconds to
/// centuries, and every value past the cap collapses onto the same run. That left the fuzzer with
/// effectively one time value on a dimension that decides whether an election completes, whether a
/// heartbeat lands before a timeout, and whether a paused server's deadline has expired.
///
/// A whole number of milliseconds also matches the clock. `Timeout` arms on millisecond
/// boundaries, so sub-millisecond precision generates values the cluster cannot distinguish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunMillis(pub u64);

impl RunMillis {
    fn duration(self) -> Duration {
        Duration::from_millis(self.0)
    }
}

impl TypeGenerator for RunMillis {
    fn generate<D: Driver>(driver: &mut D) -> Option<Self> {
        (0..=MAX_RUN_MILLIS).generate(driver).map(RunMillis)
    }

    fn mutate<D: Driver>(&mut self, driver: &mut D) -> Option<()> {
        (0..=MAX_RUN_MILLIS).mutate(driver, &mut self.0)
    }
}

#[derive(Debug, TypeGenerator)]
pub enum Operation {
    // -----------------
    // Bootstrap
    // -----------------

    // Create cluster of n Nodes. Election should happen naturally as we drive the system.
    // TODO: increase to u16
    // Create(u8),

    // -----------------
    // Normal operation
    // -----------------

    // Get client request. The command stays a full u8 so two entries are unlikely to collide,
    // since the safety checks compare entries and identical commands would hide a divergence.
    ClientRequest(ServerPick, u8),

    // Run the cluster forward for a span of simulated time.
    RunUntil(RunMillis),

    // -----------------
    // Server Faults
    // -----------------

    // Idx of the server to Pause.
    Pause(ServerPick),

    // Resume a paused server.
    Resume(ServerPick),
    //
    // // Crash a server so that all state is lost.
    // Crash(u16),
    //
    // // Restart a crashed server.
    // Restart(u16),
}

impl Operation {
    /// Perform a single Operation against the cluster.
    pub async fn apply(&self, cluster: &mut Cluster) {
        match self {
            // Operation::Create(n) => todo!(),
            Operation::ClientRequest(idx, command) => {
                // A request to a Follower is answered with a redirect rather than an entry, which is
                // itself worth exercising, so the response is intentionally discarded.
                cluster.client_request(idx.resolve(cluster.server_count()), *command);
            }
            Operation::RunUntil(millis) => {
                cluster.run_for(millis.duration()).await;
            }
            // A paused server keeps everything it held. A true crash that loses unpersisted state
            // is a separate operation and does not exist yet.
            Operation::Pause(idx) => {
                cluster.pause(idx.resolve(cluster.server_count()));
            }
            Operation::Resume(idx) => {
                cluster.resume(idx.resolve(cluster.server_count()));
            }
        }
    }
}
