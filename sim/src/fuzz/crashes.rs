use crate::fuzz::{
    execute,
    operation::{RunMillis, ServerPick, MAX_RUN_MILLIS},
    Operation,
};

// Fuzz discovered assertion failure. It was the oracle that was wrong, not Raft.
//
// assertion `left == right` failed: logs diverge between server 0 and server 1
//  left: [Entry { term: Term(2), command: 0 }]
//  right: [Entry { term: Term(1), command: 191 }]
//
// Two servers held different entries at index 1 under different terms, neither committed. Log
// Matching says nothing about that case, and the old assertion had dropped the same-term
// condition. Reaching it at all also needed a paused Leader to accept a client request.
//
// Kept because the trace exercises a Leader change with a stranded ex-Leader, which is worth
// running. It cannot fail again on its own, so it does not guard either fix.
#[test]
fn sim_bug_divergence_between_uncommitted_entries() {
    let operations = vec![
        Operation::Pause(ServerPick(4)),
        Operation::RunUntil(RunMillis(MAX_RUN_MILLIS)),
        Operation::Pause(ServerPick(1)),
        Operation::ClientRequest(ServerPick(1), 191),
        Operation::RunUntil(RunMillis(MAX_RUN_MILLIS)),
        Operation::ClientRequest(ServerPick(0), 0),
    ];

    execute(&operations);
}
