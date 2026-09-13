use crate::cluster::Cluster;
use raft_rs::state::log::Idx;

/// Several commands commit, and every server applies them in the same order.
///
/// Test that commitIdx jumping by more than one still works correctly. All 4 commands are
/// submitted before any of them replicate, so a Follower learns about all 4 commands.
#[tokio::test]
async fn commit_multiple_entries_in_order() {
    let mut cluster = Cluster::new(3);
    let leader = cluster.elect().await;

    let commands = [10, 20, 30, 40];

    // Submit all the commands to the leader.
    for command in commands {
        cluster.client_request(leader, command);
    }

    // Run until every server, not just the Leader, has committed through the last entry.
    let last_idx = Idx::from(commands.len() as u64);
    assert!(
        cluster
            .run_until_condition(|c|
                // Check that for all servers all commands have been committed
                c.idxs().all(|idx| {
                    c.commit_idx(idx) == last_idx
                }
            ))
            .await,
        "entries never committed on every server"
    );

    for idx in cluster.idxs() {
        // Every log holds the same commands in the order they were submitted.
        let log = cluster.log_entries(idx);
        let commands_in_log: Vec<u8> = log.iter().map(|entry| entry.command).collect();
        assert_eq!(commands_in_log, commands.to_vec(), "server {idx} log order");

        // The StateMachine holds the commands themselves, in apply order.
        assert_eq!(
            cluster.applied_commands(idx),
            commands.to_vec(),
            "server {idx} applied commands"
        );

        // A contiguous 1..=4 says nothing was skipped or applied twice, which is what commitIdx
        // jumping by more than one puts at risk.
        let expected: Vec<Idx> = (1..=commands.len() as u64).map(Idx::from).collect();
        assert_eq!(
            cluster.query_state_machine(idx),
            expected,
            "server {idx} apply order"
        );
    }
}

/// Committed entries survive the Leader that created them.
///
//% Compliance:
//% Leader Completeness: if a log entry is committed in a given term, then that entry will be
//% present in the logs of the leaders for all higher-numbered terms
#[tokio::test]
async fn committed_entries_survive_leader_crash() {
    let mut cluster = Cluster::new(3);
    let old_leader = cluster.elect().await;
    let old_term = cluster.current_term(old_leader);

    let commands = [10, 20, 30];
    for command in commands {
        cluster.client_request(old_leader, command);
    }

    let last_idx = Idx::from(commands.len() as u64);
    assert!(
        cluster
            .run_until_condition(|c| c.idxs().all(|idx| c.commit_idx(idx) == last_idx))
            .await,
        "entries never committed on every server"
    );

    // Lose the Leader. The survivors elect a new one in a higher term.
    cluster.crash(old_leader);
    let new_leader = cluster.elect().await;
    assert!(cluster.current_term(new_leader) > old_term);

    // The committed prefix is intact on both survivors, commands and apply order alike.
    for idx in cluster.idxs().filter(|idx| *idx != old_leader) {
        assert_eq!(
            cluster.applied_commands(idx),
            commands.to_vec(),
            "server {idx} lost committed commands across the term change"
        );
        assert_eq!(cluster.commit_idx(idx), last_idx, "server {idx} commit_idx");
    }
    cluster.assert_logs_match();

    // And the new Leader can still commit, on top of the old Leader's entries.
    cluster.client_request(new_leader, 40);
    let next_idx = last_idx + 1;
    assert!(
        cluster
            .run_until_condition(|c| c.commit_idx(new_leader) == next_idx)
            .await,
        "new Leader could not commit"
    );
    assert_eq!(cluster.applied_commands(new_leader), vec![10, 20, 30, 40]);
}
