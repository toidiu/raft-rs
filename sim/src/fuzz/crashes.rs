use crate::fuzz::{
    execute,
    operation::{RunMillis, ServerPick, MAX_RUN_MILLIS},
    Operation,
};

// Fuzz discovered panic.
//
// thread 'fuzz::tests::bla' (510311) panicked at sim/src/cluster/network.rs:114:45:
// server should send valid Packets: UnexpectedEof(1)
// note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
#[test]
fn bug_panics_on_decoding_partial_packets() {
    let operations = vec![
        Operation::RunUntil(RunMillis(MAX_RUN_MILLIS)),
        Operation::ClientRequest(ServerPick(1), 26),
        Operation::ClientRequest(ServerPick(1), 26),
        Operation::ClientRequest(ServerPick(1), 91),
        Operation::RunUntil(RunMillis(0)),
    ];

    execute(&operations);
}

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

// Fuzz discovered panic.
//
// panicked at src/mode/leader.rs:217:17:
// assertion failed: rpc.term() != &raft_state.current_term
#[test]
fn bug_two_leaders_in_one_term() {
    let operations = vec![
        Operation::Pause(ServerPick(2)),
        Operation::Pause(ServerPick(5)),
        Operation::Pause(ServerPick(4)),
        Operation::RunUntil(RunMillis(496)),
        Operation::Resume(ServerPick(5)),
        Operation::Pause(ServerPick(1)),
        Operation::Resume(ServerPick(2)),
        Operation::RunUntil(RunMillis(698)),
        Operation::Resume(ServerPick(4)),
        Operation::Resume(ServerPick(1)),
        Operation::RunUntil(RunMillis(0)),
    ];

    execute(&operations);
}
