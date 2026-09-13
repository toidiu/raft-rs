//! Drive the cluster with a generated sequence of operations and look for a broken invariant.
//!
//! The simulator is async only because the clock it runs on lives inside a tokio runtime. That is
//! confined to [`Operation::apply`] and the `block_on` around it, so an Operation stays plain data
//! that bolero can generate, shrink, and print.

use crate::{
    cluster::Cluster,
    fuzz::operation::{Operation, CLUSTER_SIZE},
};

mod operation;

#[ignore = "currently discovers packet fragment bugs very quickly"]
#[test]
fn raft() {
    bolero::check!()
        .with_type::<Vec<Operation>>()
        .for_each(|operations| execute(operations))
}

pub fn execute(operations: &[Operation]) {
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
}
