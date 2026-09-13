use crate::cluster::Cluster;
use raft_rs::server::ClientResponse;
use std::time::Duration;

/// 3 nodes, empty logs. Left alone, one wins the timeout race.
#[tokio::test]
async fn leader_emerges() {
    let mut cluster = Cluster::new(3);

    // Nobody starts as Leader.
    assert_eq!(cluster.leader(), None);

    let leader = cluster.elect().await;

    //% Compliance:
    //% wins election: receives majority of votes in cluster (ensures a single winner)
    for idx in cluster.idxs() {
        if idx != leader {
            assert!(
                cluster.is_follower(idx),
                "server {idx} should be a Follower"
            );
        }
    }

    // The heartbeat carried the winning term to everyone.
    let term = cluster.current_term(leader);
    for idx in cluster.idxs() {
        assert_eq!(cluster.current_term(idx), term, "server {idx} term");
    }

    // Nothing was appended, so every log is still empty.
    for idx in cluster.idxs() {
        assert!(cluster.log_entries(idx).is_empty(), "server {idx} log");
    }
    cluster.assert_logs_match();
}

/// Only a Leader accepts commands. Everyone else points the client elsewhere.
#[tokio::test]
async fn client_request_redirects_to_leader() {
    let mut cluster = Cluster::new(3);

    // Before any election a Follower has never heard from a Leader.
    let any = cluster.idxs().next().unwrap();
    assert_eq!(
        cluster.client_request(any, 7),
        Some(ClientResponse::Redirect(None))
    );

    let leader = cluster.elect().await;
    let follower = cluster.idxs().find(|idx| *idx != leader).unwrap();

    // The heartbeat taught the Followers who leads.
    assert_eq!(
        cluster.client_request(follower, 7),
        Some(ClientResponse::Redirect(Some(cluster.as_peer_id(leader))))
    );
}

/// A healthy Leader holds power indefinitely.
///
/// Two things have to hold for this:
/// - A Leader must heartbeat well inside the election timeout, and
/// - A Follower must re-arm its own timer when a heartbeat lands.
#[tokio::test]
async fn leader_is_stable() {
    let mut cluster = Cluster::new(3);
    let leader = cluster.elect().await;
    let term = cluster.current_term(leader);

    // Many election timeouts worth of simulated time.
    cluster.run_for(Duration::from_secs(10)).await;

    assert_eq!(
        cluster.leader(),
        Some(leader),
        "Leader changed while every server was healthy"
    );
    for idx in cluster.idxs() {
        assert_eq!(
            cluster.current_term(idx),
            term,
            "server {idx} term moved without cause"
        );
    }
}

/// Stop the Leader and the survivors elect a new one on their own.
///
/// Every server voted in the first election. The voted_for state should be cleared so a new
/// election has a chance to pick a new leader.
#[tokio::test]
async fn reelection_after_leader_pause() {
    let mut cluster = Cluster::new(3);
    let old_leader = cluster.elect().await;
    let old_term = cluster.current_term(old_leader);

    cluster.pause(old_leader);

    // No test choreography: heartbeats stop, a survivor's election timeout expires, and it
    // campaigns. Quorum of 3 is 2, which the two survivors still have.
    let new_leader = cluster.elect().await;

    assert_ne!(new_leader, old_leader);
    assert!(
        cluster.current_term(new_leader) > old_term,
        "a new Leader must serve a higher term"
    );

    // The paused server is frozen in the old term, still believing it leads.
    assert!(cluster.is_leader(old_leader));
    assert_eq!(cluster.current_term(old_leader), old_term);
}
