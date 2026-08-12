use crate::cluster::Cluster;
use raft_rs::state::log::Idx;
use std::time::Duration;

/// Several commands commit, and every server applies them in the same order.
///
/// All four commands are submitted before any of them replicate, so they travel together. One
/// AppendEntriesResp then acknowledges several entries at once and commitIdx moves by more than
/// one in a single call.
///
/// The jump is legal. What must still hold is that every index in between reaches the StateMachine
/// exactly once, in order.
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

        // A contiguous 1..=4 says nothing was skipped or applied twice. That is the risk when
        // commitIdx jumps, because a single call has to walk every index in between.
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
/// Entries are committed, the Leader is stopped, and the survivors elect a new Leader in a higher
/// term. The committed prefix must still be there afterwards, and the new Leader must be able to
/// append on top of entries it never created.
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

/// An entry does not commit without a quorum.
///
/// A Leader that has lost contact with a majority can still accept client commands and append them
/// to its own log. Nothing stops it. What must not happen is that it reports them committed.
///
/// This is the safety half of replication. `commit_idx` is a promise that a majority holds the
/// entry, and a Leader alone cannot make that promise.
#[tokio::test]
async fn no_commit_without_quorum() {
    let mut cluster = Cluster::new(3);
    let leader = cluster.elect().await;

    // Take down both Followers. The Leader is 1 of 3, short of the quorum of 2.
    let followers: Vec<_> = cluster.idxs().filter(|idx| *idx != leader).collect();
    for idx in followers {
        cluster.crash(idx);
    }

    // The Leader keeps heartbeating into the void for a long stretch of simulated time. If it were
    // going to commit wrongly, this is where it would.
    cluster.client_request(leader, 99);
    cluster.run_for(Duration::from_secs(5)).await;

    // The entry is in the Leader's log. Accepting it is fine.
    assert_eq!(cluster.log_entries(leader).len(), 1);
    assert_eq!(cluster.log_entries(leader)[0].command, 99);

    // But nothing acknowledged it, so it never commits and never reaches the StateMachine.
    assert_eq!(cluster.commit_idx(leader), Idx::initial());
    assert!(cluster.applied_commands(leader).is_empty());
}

/// A Follower that was down catches up on everything it missed.
///
/// A Follower is stopped, the remaining quorum commits several entries without it, and then it
/// comes back. Raft repairs it through the same AppendEntries path it uses for new entries. The
/// Leader walks `next_idx` back until the logs agree, then ships the tail.
///
/// The repair has to be complete rather than partial. The Follower must end up holding every entry
/// it missed, applied in the same order as everyone else, not just the newest one.
#[tokio::test]
async fn crashed_follower_catches_up() {
    let mut cluster = Cluster::new(3);
    let leader = cluster.elect().await;
    let lagging = cluster.idxs().find(|idx| *idx != leader).unwrap();

    cluster.crash(lagging);

    // With one Follower left the Leader still has a quorum of 2, so these commit while the third
    // server knows nothing about them.
    let commands = [1, 2, 3];
    for command in commands {
        cluster.client_request(leader, command);
    }
    let last_idx = Idx::from(commands.len() as u64);
    assert!(
        cluster
            .run_until_condition(|c| c.commit_idx(leader) == last_idx)
            .await,
        "entries never committed on the Leader"
    );
    assert!(cluster.log_entries(lagging).is_empty());

    // Bring it back. Its election timeout expired while it was down, so it campaigns immediately.
    // The Leader's higher term puts it back to Follower and the repair follows.
    cluster.restart(lagging);
    assert!(
        cluster
            .run_until_condition(|c| c.commit_idx(lagging) == last_idx)
            .await,
        "restarted Follower never caught up"
    );

    assert_eq!(cluster.applied_commands(lagging), commands.to_vec());
    cluster.assert_logs_match();
}

/// A Follower holding a conflicting entry has it overwritten, not merged.
///
/// A Leader accepts a command and stops before replicating it. The rest of the cluster elects a new
/// Leader and commits a different command at that same index. Two servers now disagree about what
/// index 1 holds.
///
/// Raft resolves this in one direction only. The Leader's log wins and the Follower truncates from
/// the first disagreement. Anything else leaves two servers permanently claiming different values
/// at the same index.
#[tokio::test]
async fn divergent_follower_log_is_repaired() {
    let mut cluster = Cluster::new(3);
    let old_leader = cluster.elect().await;

    // Accept a command, then lose the Leader before a single AppendEntries leaves it. The entry
    // exists only here, uncommitted, and no other server has any idea it was written.
    cluster.client_request(old_leader, 111);
    cluster.crash(old_leader);
    assert_eq!(cluster.log_entries(old_leader).len(), 1);

    // The survivors elect among themselves and commit a different command at the same index.
    let new_leader = cluster.elect().await;
    cluster.client_request(new_leader, 222);
    assert!(
        cluster
            .run_until_condition(|c| c.commit_idx(new_leader) == Idx::from(1))
            .await,
        "new Leader could not commit"
    );

    // Bring the old Leader back. Index 1 now disagrees. It holds 111 from the old term, while the
    // rest of the cluster holds the committed 222 from the new one.
    cluster.restart(old_leader);
    assert!(
        cluster
            .run_until_condition(
                |c| c.log_entries(old_leader).first().map(|e| e.command) == Some(222)
            )
            .await,
        "conflicting entry was never replaced"
    );

    // The conflicting entry is gone rather than appended after, so the log did not grow.
    assert_eq!(cluster.log_entries(old_leader).len(), 1);
    cluster.assert_logs_match();
}
