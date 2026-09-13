# Bugs found by the cluster simulation

Protocol defects in existing code, surfaced by running three servers against a simulated clock.
None were visible to the unit tests, which drive one `Mode` in isolation and never let time pass.

Fix one at a time. Each row lands with the test that proves it.

| # | Bug | Where | Effect | Test |
|---|---|---|---|---|
| 1 | Follower does not reset its election timer on an accepted AppendEntries | `mode/follower.rs` | Follower campaigns against a healthy Leader as soon as its own timeout fires | `leader_is_stable` |
| 2 | Leader has no heartbeat interval — re-arms from the same 150-300ms election range | `timeout.rs`, `mode/leader.rs` | A Leader that drew 280ms loses a Follower that drew 160ms | `leader_is_stable` |
| 3 | `votedFor` is never cleared when the term advances | `state/raft_state.rs`, `mode/mod.rs` | After one election every server holds a vote forever, so no second Leader can ever be elected | `reelection_after_leader_crash` |
| 4 | `update_commit_idx` asserts commit advances by exactly 1 | `state/raft_state.rs` | Panics when a peer acknowledges 2+ entries at once, or a Follower catches up | `commit_multiple_entries_in_order` |

## Order of work

**1 + 2 together.** Neither is provable alone through `leader_is_stable`: the Follower reset does
not help if the Leader heartbeats too slowly, and the heartbeat does not help if Followers never
reset. They can be split if the Follower reset is pinned separately by asserting its
`timeout_deadline` moves after an accepted AppendEntries.

Everything longer-running depends on a Leader that can hold power, so this goes first.

Prerequisite: `leader_is_stable` needs `Cluster::run_for`, which was removed. It comes back here.

**3, then 4.**

---

## 1. Follower does not reset its election timer

`mode/follower.rs`, in `on_recv_append_entries`.

//% Compliance:
//% If election timeout elapses without receiving AppendEntries RPC from current leader or
//% granting vote to candidate: convert to candidate

A heartbeat landing means the Leader is alive, so the Follower must re-arm. Reset only on an
*accepted* AppendEntries — a rejected one does not establish that the sender leads.

## 2. Leader has no heartbeat interval

//% Compliance:
//% Upon election: send initial empty AppendEntries RPCs (heartbeat) to each server; repeat during
//% idle periods to prevent election timeouts (§5.2)

Raft needs the heartbeat interval well under the election timeout. Re-arming a Leader from the same
150-300ms range breaks that: the two draws are independent, so a Leader can be slower than a
Follower.

Add `HEARTBEAT_DURATION` and `Timeout::reset_heartbeat()`, called from `Leader::on_leader` and
`Leader::on_timeout`. `on_timeout` matters because the auto-rearm in `TimeoutReady::poll` already
drew from the election range by the time the Leader runs.

Open question: should the interval be derived from `MIN_REARM_DURATION` so the invariant is
enforced rather than assumed?

## 3. `votedFor` is never cleared when the term advances

`mode/mod.rs::on_recv` sets `raft_state.current_term = *rpc.term()` and leaves `voted_for` alone.

//% Compliance:
//% `votedFor` candidateId that received vote in current term (or null if none)

A vote is scoped to a term. Carrying it forward means one stale vote denies every future election,
since a server that already voted refuses to vote again. **The cluster cannot elect a second
Leader.**

Two places to clear it: adopting a higher term seen on the wire, and starting an election.

Related: `mode/candidate.rs::start_election` records its self-vote only in `votes_received`, not in
`raft_state.voted_for`, so a Candidate can also vote for a rival in the same term.

## 4. `update_commit_idx` asserts commit advances by exactly 1

`state/raft_state.rs`

```rust
assert!(
    commit_up_to_idx == self.commit_idx + 1,
    "Comitting more than 1 entry!! its not wrong but understand this path."
);
```

Commit routinely jumps by more than one: a Leader whose peer acknowledges 2 entries at once, or a
Follower computing `min(leader_commit, last_idx)` while several behind.

The property the assert was reaching for — every entry between old and new commit reaches the state
machine, none skipped — is already guaranteed by the `while` loop below it, which walks one index
at a time regardless of the jump. Drop the assert, keep the monotonicity one.

---

## Known but not yet actionable

**Granting a vote does not reset the election timer** (§5.2). Spec-required, but not observable
under the current model: packet passes do not advance the clock, so elections resolve within a
single instant and a voter cannot time out mid-election. Testable once the network can delay a
packet.
