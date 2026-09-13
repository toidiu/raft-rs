//! Raft at a cluster size where one round outgrows a single read of the IO buffer.

use crate::cluster::Cluster;
use raft_rs::state::log::Idx;

// Large enough that one broadcast exceeds the byte cap on a single queue read. Below this a whole
// round fits in one read, and the framing between a server and the network is never exercised.
const CLUSTER_SIZE: usize = 25;

// Few enough that every AppendEntries stays small. The pressure under test is the number of
// packets in a round, not the number of entries in a packet.
const COMMANDS: [u8; 3] = [10, 20, 30];

/// A 25 server cluster elects a Leader and commits commands on every server.
///
/// Twenty five servers start together and take three client commands. Nothing is paused and no
/// packet is lost, so the only thing separating this from the 3 server tests is the size of a
/// single round.
///
/// Size is what pushes the byte stream between a server and the network past one read. A broadcast
/// is 24 packets and a Leader takes 24 responses at once, both beyond what one read returns. A
/// packet landing on a read boundary is split in half, and whoever decodes it has to put it back
/// together. At 3 servers the whole round fits in one read, so nothing ever asks that question and
/// a decoder that assumes one read holds whole packets looks correct.
#[ignore = "fails until packet framing is fixed"]
#[tokio::test]
async fn large_cluster_replicates() {
    let mut cluster = Cluster::new(CLUSTER_SIZE);
    let leader = cluster.elect().await;

    // Every other server accepted the Leader, so the election carried across the full fan-out.
    for idx in cluster.idxs().filter(|idx| *idx != leader) {
        assert!(
            cluster.is_follower(idx),
            "server {idx} should be a Follower"
        );
    }

    // Ordinary replication. The Leader broadcasts to 24 peers and needs 13 of them to acknowledge.
    for command in COMMANDS {
        cluster.client_request(leader, command);
    }

    let last_idx = Idx::from(COMMANDS.len() as u64);
    assert!(
        cluster
            .run_until_condition(|c| c.idxs().all(|idx| c.commit_idx(idx) == last_idx))
            .await,
        "entries never committed on every server"
    );

    // Every server holds the same commands in the order they were submitted, and applied them.
    for idx in cluster.idxs() {
        let log: Vec<u8> = cluster
            .log_entries(idx)
            .iter()
            .map(|entry| entry.command)
            .collect();
        assert_eq!(log, COMMANDS.to_vec(), "server {idx} log order");
        assert_eq!(
            cluster.applied_commands(idx),
            COMMANDS.to_vec(),
            "server {idx} applied commands"
        );
    }
}
