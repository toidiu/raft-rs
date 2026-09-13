# Integration test plan

Unit tests today drive one `Mode` in isolation with a `MockIo`. They prove each
branch in isolation. They cannot prove that a cluster elects a leader, replicates
an entry, and commits it — which is the only thing that says Raft works.

This is the work needed to get there, in dependency order. Phases 1-3 are
prerequisites; the tests themselves are phase 5.

## Checklist

**Phase 1 — unblock**

- [ ] 1.1 `set_commit_idx` panics when commit advances by more than 1
- [ ] 1.2 All servers share one fixed 200ms election timeout, so elections never resolve
- [ ] 1.3 `Server` is private and unconstructible from a test harness
- [ ] 1.4 Decide where integration tests live (this gates 1.3)
- [ ] 1.5 Delete or fix `src/server/peer.rs` — dead code that no longer compiles

**Phase 2 — client API**

- [ ] 2.1 `Leader::on_client_request` — append to the log, replicate
- [ ] 2.2 Followers track the current leader for redirects
- [ ] 2.3 `Server::on_client_request` — the entry point, dispatching by mode
- [ ] 2.4 Decide how a client learns its request committed

**Phase 3 — harness**

- [ ] 3.1 `Cluster` type: N servers, wired together
- [ ] 3.2 Packet router: drain each egress, dispatch by `Packet::to()`
- [ ] 3.3 Deterministic stepping — no wall clock, no task scheduling races
- [ ] 3.4 Convergence helper: `run_until(cond)` with a bounded step budget

**Phase 4 — observability**

- [ ] 4.1 `Mode::is_leader()` / `current_mode()`
- [ ] 4.2 Read the applied entries out of `StateMachine`
- [ ] 4.3 Log inspection available to integration tests
- [ ] 4.4 A `Cluster::assert_logs_match()` invariant check

**Phase 5 — the tests**

- [ ] 5.1 Single election: 3 nodes, one becomes leader
- [ ] 5.2 Replicate one entry to all followers
- [ ] 5.3 Commit: entry reaches the state machine on a quorum
- [ ] 5.4 Repair a divergent follower log
- [ ] 5.5 Leader failure, re-election, continuity
- [ ] 5.6 No commit without a quorum

**Phase 6 — later**

- [ ] 6.1 Packets larger than `IO_BUF_LEN`
- [ ] 6.2 Fault injection: drop, delay, reorder, duplicate
- [ ] 6.3 Partition and heal
- [ ] 6.4 Randomized/seeded fuzz loop

---

## Phase 1 — unblock

These are defects in existing code. Each one will stop the first cluster test
from running, so they come first.

### 1.1 `set_commit_idx` panics when commit advances by more than 1

`src/raft_state.rs:64-76`

```rust
pub fn set_commit_idx(&mut self, idx: Idx, peer_id: PeerId, mode: CurrentMode) {
    assert!(idx >= self.commit_idx, "commitIdx is monotonically increasing");
    if idx > self.commit_idx {
        assert!(
            idx == self.commit_idx + 1,
            "we expect commitIdx should increase by 1 so each entry is captured in the log"
        );
    }
```

**Why it breaks:** commit routinely jumps by more than one.

- A Leader whose peer acknowledges 2 entries at once calls `update_commit_idx`
  with `match_idx = 2` while `commit_idx == 0`. Panic.
- A Follower catching up computes `min(leader_commit, last_idx)`
  (`src/mode/follower.rs:108-111`), which can be several ahead of its own
  `commit_idx`. Panic.

The intent behind the assert is sound — every entry between the old and new
commit index must be applied to the state machine, none skipped. But the loop
below it already guarantees that:

```rust
while self.commit_idx() > self.last_applied() {
    self.last_applied += 1;
    // ... apply log[last_applied]
}
```

It walks one index at a time regardless of how far `commit_idx` jumped. The
assert is enforcing a property the loop does not need.

**Fix:** drop the `+1` assert, keep the monotonicity assert. The `while` loop is
the real guarantee.

**Test to add:** set commit_idx from 0 to 3 in one call on a 3-entry log, assert
all three entries reached the state machine in order. That is the property the
assert was reaching for, tested directly.

### 1.2 One fixed election timeout for every server

`src/timeout.rs:14-17` and `:88-97`

```rust
const MIN_REARM_DURATION: u64 = 150;
const MAX_REARM_DURATION: u64 = 300;
const TEST_REARM_DURATION: u64 = 200;

fn rearm_duration<R: RngCore>(prng: &mut R) -> Duration {
    cfg_if::cfg_if! {
        if #[cfg(test)] {
            let range = TEST_REARM_DURATION;
            Duration::from_millis(range)
        } else {
            let range = prng.gen_range(MIN_REARM_DURATION..=MAX_REARM_DURATION);
            Duration::from_millis(range)
        }
    }
}
```

Under `cfg(test)` every timeout is exactly 200ms and the `prng` is ignored. Unit
tests want that determinism. A cluster cannot survive it.

**Why it breaks:** randomized election timeouts are not a detail, they are how
Raft breaks symmetry (§5.2). With identical timeouts all three servers time out
on the same tick, all become candidates, all vote for themselves, nobody wins a
quorum, the term increments, and it repeats forever. The test hangs or exhausts
its step budget with no leader.

**Fix:** keep determinism, drop the uniformity. The `prng` is already threaded
through `Timeout::new` (`src/timeout.rs:56`) and each server can be seeded
differently — the machinery exists, `cfg(test)` just bypasses it.

Options:

1. **Seeded range under test.** Use `prng.gen_range` in test builds too, and give
   each server a different seed in the harness. Deterministic per seed, distinct
   per server. Least invasive.
2. **Injectable duration.** `Timeout::new_with_duration(prng, Duration)`, so the
   harness sets node 1 to 150ms, node 2 to 200ms, node 3 to 250ms. Fully explicit
   — a test can force a specific server to win by giving it the shortest timeout.

Option 2 is worth the small API addition. Being able to say "node 2 wins this
election" makes 5.5 (leader failure) writable without racing.

Note `TEST_REARM_DURATION` is also what makes the existing unit tests
predictable. Keep them working — check every `tokio::time::advance` call site
before changing the default.

### 1.3 `Server` is private

`src/server.rs:18` — `struct Server` with no `pub`, and `mod server;` in
`src/lib.rs:10` is private too. Its fields (`mode`, `state`, `peer_list`) are
private, and `Server::new` is private.

A harness needs to construct servers, hold them, step them, and inspect their
state.

**Fix depends on 1.4.** If integration tests live inside `src/`, `pub(crate)` is
enough. If they live in `tests/`, `Server` and a meaningful slice of its API must
be `pub`, which means committing to a public API earlier than you may want.

### 1.4 Decide where integration tests live

This gates 1.3 and 4.3, so decide it first.

**The constraint:** `#[cfg(test)]` items are compiled only for the crate's own
test build. Files in `tests/` are separate crates and see only the *public* API.
So `Log::test_len` (`src/log.rs:171`), `Log::test_get_unchecked`,
`Rpc::test_recv_new_append_entry`, `MockIo`, and `ServerEgress::send_raw` are all
invisible from `tests/`.

| | in `src/` (e.g. `src/tests/cluster.rs`) | in `tests/cluster.rs` |
|---|---|---|
| Existing `#[cfg(test)]` helpers | available | invisible |
| Visibility needed | `pub(crate)` | `pub` |
| Forces public API design | no | yes |
| Tests what a user can actually do | no | yes |
| `cargo test` runs it | yes | yes |

**Recommendation: start in `src/tests/`.** The public API does not exist yet —
there is no `lib.rs` export surface at all beyond `fn start()`
(`src/lib.rs:14`). Designing one *because* the test directory demands it is
backwards. Move to `tests/` once the client API in phase 2 has settled and you
want to pin the external contract.

Concretely: add `#[cfg(test)] mod tests;` to `src/lib.rs`, with
`src/tests/mod.rs` declaring `mod cluster;`, `mod harness;`.

### 1.5 `src/server/peer.rs` is dead code

It imports `crate::io::ServerEgress` and `crate::rpc::Rpc` — modules that no
longer exist (they are `crate::queue` and `crate::packet` now). It compiles only
because nothing declares `mod peer;` — `src/server.rs:14` declares only `mod id`.

`PeerInfo::mock_list` in its test module looks like it was meant for exactly the
kind of multi-node setup this plan needs. Either revive it against the current
module names or delete it. Leaving a stale file that shadows the concept will
confuse whoever writes the harness.

---

## Phase 2 — client API

Nothing currently appends to a Leader's log. Every entry in every test is
injected by hand via `update_to_match_leaders_log`. Without a client API there is
no way to test replication end to end, and `bugs.md` bug 7 notes several
behaviors that are only correct *because* a Leader's log never grows.

### 2.1 `Leader::on_client_request`

```rust
//% Compliance:
//% If command received from client: append entry to local log, respond after entry applied to
//% state machine (§5.3)
pub fn on_client_request<E: ServerEgress>(
    &mut self,
    server_id: &ServerId,
    peer_list: &[PeerId],
    command: Command,
    raft_state: &mut RaftState,
    io_egress: &mut E,
) -> Idx {
    raft_state
        .log
        .push(vec![Entry::new(raft_state.current_term, command)]);
    let idx = raft_state.log.last_idx();

    // Replicate immediately rather than waiting for the next heartbeat.
    self.broadcast_send_append_entries(server_id, peer_list, raft_state, io_egress);

    idx
}
```

Returns the `Idx` the command landed at — the handle a client needs to know when
its request committed.

**This is what makes the `log.last_idx()` stand-in in bug 7 wrong.** Once the
Leader's log grows between sending an RPC and receiving its response, that
approximation breaks. The `entries_cnt` field already fixed it — this is the
change that would have exposed it.

**Watch:** `Log::push` (`src/log.rs:37-41`) appends blindly. That is right for a
Leader, which is the only writer of its own log. Don't route this through
`update_to_match_leaders_log`, which is the Follower's conflict-resolution path.

`Command` is `u8` (`src/log/entry.rs:4`). Fine for tests. Note it if you ever
want real payloads.

### 2.2 Followers track the current leader

`src/mode/follower.rs:63-69` destructures `leader_id: _` and throws it away.

A client that contacts a Follower must be redirected — that is the stated purpose
of the field (`src/packet/append_entries.rs:18`: "leaderId: so follower can
redirect clients"). Store it:

```rust
pub struct Follower {
    // The Leader this Follower last accepted an AppendEntries from, for client redirects.
    current_leader: Option<PeerId>,
}
```

Set it whenever an AppendEntries is accepted; clear it on mode transition. Note
`Follower` is currently a unit struct constructed as `Follower` in several places
(`src/mode.rs:53`, `src/mode/follower.rs:148`) — adding a field touches those.

### 2.3 `Server::on_client_request`

```rust
pub fn on_client_request(&mut self, command: Command) -> ClientResult {
    match &mut self.mode {
        Mode::Leader(leader) => {
            let idx = leader.on_client_request(
                &self.server_id, &self.peer_list, command,
                &mut self.state, &mut self.io_egress,
            );
            ClientResult::Accepted(idx)
        }
        Mode::Follower(f) => ClientResult::Redirect(f.current_leader()),
        Mode::Candidate(_) => ClientResult::NoLeader,
    }
}
```

Three outcomes, all of which a test wants to assert: accepted with an index,
redirected to a known leader, or no leader right now (mid-election).

### 2.4 How a client learns its request committed

Raft says respond *after the entry is applied to the state machine*. Options, in
increasing order of effort:

1. **Poll.** The test holds the `Idx` from `Accepted(idx)` and waits for
   `raft_state.last_applied() >= idx`. Zero production code. Good enough for
   every test in phase 5. **Start here.**
2. **Pending-request map.** `BTreeMap<Idx, Waker/oneshot>` on the Leader,
   resolved in `set_commit_idx`'s apply loop. This is what a real client API
   needs, but it is not needed to test replication.

Do not build option 2 to write these tests. Note it and move on.

---

## Phase 3 — harness

### 3.1 `Cluster`

```rust
struct Cluster {
    servers: Vec<Server>,
    queues: Vec<NetworkQueueImpl>,   // one per server, index-aligned
    ids: Vec<ServerId>,
}

impl Cluster {
    fn new(n: usize) -> Cluster;

    fn leader(&self) -> Option<usize>;              // index of the current Leader
    fn step(&mut self);                             // one full round: deliver, then process
    fn tick(&mut self);                             // fire election timeouts
    fn run_until(&mut self, cond: impl Fn(&Cluster) -> bool) -> bool;
}
```

`Server::new` (`src/server.rs:42-61`) already returns
`(Server, NetworkQueueImpl)`, so construction is straightforward. Each server
needs a `peer_list` of the *other* servers' ids — note the ids are `ServerId` for
self and `PeerId` for others, and the two are distinct types over the same 16
bytes (`src/server/id.rs`). The harness will convert; a helper is worth writing
once.

### 3.2 The router

This is the piece that does not exist. Each server has its own ingress/egress
byte queues; nothing connects one server's egress to another's ingress.

```rust
fn route(&mut self) {
    // Drain every server's egress first, then deliver. Draining and delivering in
    // one pass would let a packet sent this round arrive in the same round.
    let mut in_flight: Vec<(Id, Vec<u8>)> = Vec::new();

    for queue in self.queues.iter_mut() {
        while let Some(bytes) = queue.get_send() {
            // One read can hold several packets.
            let mut buf = DecoderBuffer::new(&bytes);
            while !buf.is_empty() {
                let (packet, rest) = Packet::decode(buf).unwrap();
                buf = rest;
                in_flight.push((packet.to(), re_encode(&packet)));
            }
        }
    }

    for (to, bytes) in in_flight {
        let idx = self.index_of(to);
        self.queues[idx].push_recv_bytes(bytes);
    }
}
```

Points that will bite:

- **`get_send` returns a raw byte blob** (`src/queue/network.rs:56-70`) which may
  contain multiple concatenated packets — `send_packet` appends to a shared
  `VecDeque` per call. You must decode in a loop, not once.
- **Decode then re-encode per packet.** The router needs `Packet::to()` to
  dispatch, and delivering the whole blob to one peer would misroute the rest.
- **`Packet::to()` returns `Id`**, the untyped form. Mapping back to a server
  index needs an `Id -> usize` table built from `as_bytes()`.
- **Two-phase delivery.** Collect everything, then deliver. Otherwise a message
  sent during this round can be processed in the same round, which hides ordering
  bugs and makes step counts meaningless.

### 3.3 Deterministic stepping

The existing `mock_event_loop` test (`src/server.rs:258-352`) spawns tokio tasks
and uses `tokio::time::advance`. It works for one server. It will be miserable
for three — task interleaving is not controlled, so failures won't reproduce.

**Drive the servers directly instead.** `Server::recv` (`:108`) and
`Server::on_timeout` (`:99`) are plain synchronous functions. A cluster test does
not need the async runtime at all:

```rust
fn step(&mut self) {
    self.route();
    for server in self.servers.iter_mut() {
        server.recv();
    }
}

fn tick(&mut self) {
    for server in self.servers.iter_mut() {
        server.on_timeout();
    }
}
```

Both are private (`fn recv`, `fn on_timeout`) — make them `pub(crate)`.

This gives a fully deterministic, single-threaded cluster: same steps, same
result, every run. Reserve the async path for a separate test that specifically
exercises the runtime wiring.

`Timeout` still needs tokio's clock because `RaftState` holds one and
`election_timer.reset()` is called on the real `Sleep`. Keep `#[tokio::test]` and
`tokio::time::pause()`, but don't depend on task scheduling for correctness.

### 3.4 `run_until`

```rust
fn run_until(&mut self, cond: impl Fn(&Cluster) -> bool) -> bool {
    const MAX_STEPS: usize = 100;
    for i in 0..MAX_STEPS {
        if cond(self) { return true; }
        self.step();
        // Fire timeouts periodically so elections can start and heartbeats flow.
        if i % 5 == 0 { self.tick(); }
    }
    false
}
```

A bounded budget turns "the cluster never converges" into a failed assertion
instead of a hung test. Return `bool` and let the caller assert, so the failure
message names what it was waiting for.

---

## Phase 4 — observability

Assertions need to see inside. All of these are test-only accessors.

- **4.1 Mode.** `Mode` (`src/mode.rs:45-49`) has no accessor. Add
  `is_leader()` / `is_candidate()` and `Cluster::leader() -> Option<usize>`.
  Needed by nearly every test.
- **4.2 State machine.** `StateMachine::entries` (`src/state_machine.rs:8`) is
  private with no reader — `apply` pushes and nothing ever looks. Add
  `applied_entries() -> &[CommitEntry]`. Without this you cannot assert an entry
  was *applied*, only that `commit_idx` moved.
- **4.3 Log.** `test_len` / `test_get_unchecked` (`src/log.rs:160-174`) are
  `#[cfg(test)]`, which is fine if tests live in `src/` (see 1.4). Add
  `entries() -> &[Entry]` for whole-log comparison between servers.
- **4.4 Invariant helper.** `Cluster::assert_logs_match()` — for every pair of
  servers, entries at the same index up to `min(commit_idx)` must be identical.
  That is the Log Matching Property (§5.3), and checking it after every test is
  worth more than any single assertion.

---

## Phase 5 — the tests

Each builds on the last. Write them in order.

### 5.1 Single election

3 servers, empty logs. Tick until one becomes Leader.

Assert: exactly one Leader; the other two are Followers; all agree on
`current_term`.

This is the first test that can hang — if 1.2 isn't fixed, it will. Bounded
`run_until` turns that into a clean failure.

### 5.2 Replicate one entry

Elect a Leader, submit one command, run until quiet.

Assert: every server's log holds exactly one entry with the same term and
command; every `next_idx` is 2; every `match_idx` is 1.

This is the first end-to-end exercise of the bug 4/5/6/7 fixes together.

### 5.3 Commit

Same setup, then assert `commit_idx == 1` on the Leader and that the entry
reached the state machine via 4.2.

Then submit 3 more commands and assert all four applied *in order*. This is where
1.1 fires if unfixed.

### 5.4 Repair a divergent follower

Hand-build a follower with entries from an older term that conflict with the
Leader's. Run.

Assert: the Leader walks `next_idx` back, the follower truncates and matches, and
`assert_logs_match` passes.

This is the cluster-level version of the bug 1 unit test — the case where
`prev = initial` on a non-empty log had to succeed. Worth having at both levels:
the unit test pins the mechanism, this one pins the outcome.

### 5.5 Leader failure and re-election

Elect, replicate, then stop stepping the Leader (simulate a crash by excluding it
from `step`/`tick`). Tick the rest.

Assert: a new Leader emerges from the remaining two, its term is higher, and the
committed entry from the old term survives.

Needs the harness to support excluding a server — plan for `Cluster::isolate(i)`
/ `heal(i)` from the start, it is cheap if designed in and awkward if bolted on.

### 5.6 No commit without a quorum

5 servers, isolate 3. The Leader has 2 of 5.

Assert: submitted entries append to the Leader's log but `commit_idx` never
advances, and nothing reaches any state machine.

This is the test that would catch a regression in the `min`/`max` clamps on
`match_idx` — an inflated `match_idx` would fake a quorum.

---

## Phase 6 — later

- **6.1 Oversized packets.** `IO_BUF_LEN` is 1024 (`src/queue.rs:25`) and
  `recv_packet` reads one bufferful (`src/queue/server_ingress.rs:65-75`). A
  Leader catching up a far-behind follower sends its whole tail in one RPC — with
  9 bytes per encoded entry, ~110 entries overflows the buffer and the packet is
  truncated mid-decode. There is already a `TODO` about handling remaining bytes
  (`src/queue/server_ingress.rs:87`). Add a test that replicates 200 entries; it
  will fail, and it should.
- **6.2 Fault injection.** The router is the natural seam: drop, delay, reorder,
  or duplicate a packet before delivery. Duplication matters most — the echo-based
  response matching (bug 6) exists precisely to handle retransmits, and nothing
  currently tests it at the cluster level.
- **6.3 Partition and heal.** Generalize 5.5/5.6 into a reachability matrix in
  the router.
- **6.4 Randomized loop.** Seeded PRNG choosing steps, timeouts, and faults, with
  `assert_logs_match` after every step. Print the seed on failure so it reproduces.

---

## Order of work

1. **1.4** — decide where tests live. Everything else depends on it.
2. **1.1, 1.2** — the two defects that make a cluster impossible.
3. **1.3, 1.5** — visibility and cleanup.
4. **3.1-3.4** — the harness, then **5.1**. First green cluster test.
5. **4.1-4.4** — accessors, as the tests need them.
6. **2.1-2.4** — the client API, then **5.2, 5.3**.
7. **5.4-5.6**, then phase 6.

The single highest-value milestone is 5.1 — three servers electing a leader over
real encoded packets. It exercises the full path (mode transitions, timeouts,
encode/decode, routing) and everything after it is incremental.
