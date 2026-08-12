#![allow(dead_code)]

//! Drive the cluster with a generated sequence of operations and look for a broken invariant.
//!
//! The simulator is async only because the clock it runs on lives inside a tokio runtime. That is
//! confined to [`Operation::apply`] and the `block_on` around it, so an Operation stays plain data
//! that bolero can generate, shrink, and print.

use crate::cluster::{node::ServerIdx, Cluster};
use bolero::generator::TypeGenerator;
use std::time::Duration;

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

    // Get client request.
    ClientRequest(u8, u8),

    // Run the cluster until some time.
    RunUntil(Duration),

    // -----------------
    // Server Faults
    // -----------------

    // Idx of the server to Pause.
    Pause(u16),

    // Resume a paused server.
    Resume(u16),
    //
    // // Crash a server so that all state is lost.
    // Crash(u16),
    //
    // // Restart a crashed server.
    // Restart(u16),
}

// pub fn buggy_add(x: u32, y: u32) -> u32 {
//     // if x == 12976 && y == 14867 {
//     //     return x.wrapping_sub(y);
//     // }
//     return x.wrapping_add(y);
// }

const CLUSTER_SIZE: usize = 5;

// need to bump MAX_EVENTS to support higher duration
const MAX_DURATION: Duration = Duration::from_secs(5);

/// Any sequence of client requests, pauses, and resumes leaves the cluster's logs consistent.
// #[ignore = "currently discovers the queue limit bugs very quickly"]
#[ignore = "currently discovers packet fragment bugs very quickly"]
#[test]
fn fuzz_raft() {
    bolero::check!()
        .with_type::<Vec<Operation>>()
        .for_each(|operations| {
            // A runtime per input, since the paused clock belongs to the runtime and every input
            // has to start from the same time. Reusing one would carry the previous input's clock.
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("failed to build the simulation runtime");

            runtime.block_on(async {
                // Cluster::new freezes the clock, so it has to run inside the runtime.
                let mut cluster = Cluster::new(CLUSTER_SIZE);
                assert_eq!(cluster.leader(), None, "nobody leads before an election");

                for operation in operations.iter() {
                    operation.apply(&mut cluster).await;
                }

                // Log Matching holds no matter which operations ran. Servers are allowed to be
                // behind, never to disagree at an index they both hold.
                cluster.assert_logs_match();
            });
        })
}

impl Operation {
    /// Perform a single Operation against the cluster.
    async fn apply(&self, cluster: &mut Cluster) {
        match self {
            // Operation::Create(n) => todo!(),
            Operation::ClientRequest(idx, command) => {
                // A request to a Follower is answered with a redirect rather than an entry, which is
                // itself worth exercising, so the response is intentionally discarded.
                cluster.client_request(Self::server_idx(*idx as usize), *command);
            }
            Operation::RunUntil(duration) => {
                cluster.run_for(*duration.min(&MAX_DURATION)).await;
            }
            // A paused server keeps everything it held. A true crash that loses unpersisted state
            // is a separate operation and does not exist yet.
            Operation::Pause(idx) => {
                cluster.pause(Self::server_idx(*idx as usize));
            }
            Operation::Resume(idx) => {
                cluster.resume(Self::server_idx(*idx as usize));
            }
        }
    }

    /// Fold a generated number onto a server that exists.
    ///
    /// Rejecting the out of range half of the input space instead would spend most generated
    /// operations on nothing.
    fn server_idx(idx: usize) -> ServerIdx {
        ServerIdx(idx % CLUSTER_SIZE)
    }
}
