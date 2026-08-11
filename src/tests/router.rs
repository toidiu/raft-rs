use crate::tests::cluster::Cluster;

/// A packet is not delivered in the pass that sends it.
///
/// The router models the wire as state, so "left the Leader" and "arrived at the Follower" are two
/// separate passes. Without that a request and its reply would resolve in one step, and the
/// ordering a test observes would not be the ordering the protocol sees.
#[tokio::test]
async fn packet_waits_a_pass_on_the_wire() {
    let mut cluster = Cluster::new(3);
    let leader = cluster.elect().await;

    // Settle any election traffic still on the wire.
    cluster.drain_network();
    assert_eq!(cluster.in_flight_count(), 0);

    // The Leader appends and broadcasts, but the packets are only queued on its egress.
    cluster.client_request(leader, 42);
    assert_eq!(cluster.in_flight_count(), 0);

    // One pass puts them on the wire. No Follower has seen them yet.
    cluster.run_next().await;
    assert_eq!(
        cluster.in_flight_count(),
        2,
        "one AppendEntries per Follower"
    );
    for idx in cluster.idxs() {
        if idx != leader {
            assert!(
                cluster.log_entries(idx).is_empty(),
                "server {idx} saw it early"
            );
        }
    }

    // The next pass delivers them.
    cluster.run_next().await;
    assert_eq!(
        cluster.in_flight_count(),
        2,
        "now the responses are on the wire"
    );
    for idx in cluster.idxs() {
        assert_eq!(cluster.log_entries(idx).len(), 1, "server {idx} log");
    }
}
