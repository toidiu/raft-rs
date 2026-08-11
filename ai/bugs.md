# Known bugs

All seven are fixed. The Leader now ships entries, and the three index
conventions below agree with each other.

Kept as a record of what was wrong and why, since the reasoning is not obvious
from the corrected code. Each section states the original defect, the failure it
caused, and the fix that landed. Ordered as they were repaired: existing code
made correct first, then replication built on top of it.

## Background

Three index conventions have to line up. Getting them straight makes every bug
below obvious.

**`Idx` is 1-indexed.** `Idx(0)` is `Idx::initial()` and names *no* entry. It is
the empty prefix that sits before the first entry. `as_log_idx()` converts to the
0-indexed `Vec` position by subtracting 1 (`src/log/idx.rs:21-25`):

```rust
pub fn as_log_idx(&self) -> usize {
    // Idx is 1 indexed while the Log.entries is 0 indexed.
    self.0 as usize - 1
}
```

That subtraction underflows on `Idx(0)`. Any path that can produce `Idx(0)` and
feed it here is a panic in debug, a huge wrong number in release.

**`next_idx` is the index of the next entry to *send*.** The peer does not have
it (`src/mode/leader.rs:16-17`). It is initialized to `last_idx + 1`
(`src/mode/leader.rs:34` and `:69`), so its range is `[1, last_idx + 1]`. It
never legitimately reaches 0.

**`prev_log_term_idx` is the entry immediately *preceding* the new ones**
(`src/packet/append_entries.rs:27-29`), i.e. `next_idx - 1`. The Follower relies
on this: it appends the first entry at `prev.idx + 1`
(`src/mode/follower.rs:97`).

So for a peer at `next_idx == N`:

```
prev_log_term_idx.idx == N - 1
entries                == log[N ..= last_idx]
```

`prev` and `next_idx` are never the same number. Wherever the code treats them
as interchangeable, it is wrong.

## Bugs

Existing code:

- [x] 1. log: `entry_matches` rejects an initial `prev` when the local log is non-empty
- [x] 2. follower: asserts on `leader_commit_idx`, a peer-supplied value
- [x] 3. leader: `next_idx` underflows to `Idx(0)` when a peer at `next_idx == 1` replies false

Leader replication:

- [x] 4. leader: sets `prev_log_term_idx` to `next_idx` instead of `next_idx - 1`
- [x] 5. leader: always sends `entries: vec![]`, so nothing ever replicates
- [x] 6. leader: response matching compares the echoed `prev` against `next_idx`
- [x] 7. leader: success path never advances `next_idx` / `match_idx`

Bugs 4-7 were one change. Fixing 4 alone makes 6 drop every response; fixing 6
alone exposes 7. They had to land together or the Leader would have gotten worse,
not better.

## Details

### 1. log: `entry_matches` rejects an initial `prev` on a non-empty log

**Where:** `src/log.rs:110-114`

```rust
pub(crate) fn entry_matches(&self, term_idx: TermIdx) -> MatchOutcome {
    // TermIdx::initial indicates that both logs are empty
    if term_idx.is_initial() && self.entries.is_empty() {
        return MatchOutcome::Match;
    }
```

**What it does:** an initial `TermIdx` returns `Match` only when this log is
also empty. Otherwise control falls to `find_entry_at`
(`src/log.rs:128-136`), which returns `None` for an initial `Idx`:

```rust
pub fn find_entry_at(&self, idx: &Idx) -> Option<&Entry> {
    if *idx == Idx::initial() {
        return None;
    }
    self.entries.get(idx.as_log_idx())
}
```

`None` becomes `MatchOutcome::DoesntExist` (`src/log.rs:122-124`), and the
Follower turns that into a false reply (`src/mode/follower.rs:79-88`):

```rust
let log_contains_matching_prev_entry = matches!(
    raft_state.log.entry_matches(*prev_log_term_idx),
    MatchOutcome::Match
);
let response = if rpc_term_lt_current_term || !log_contains_matching_prev_entry {
    false
```

**Why it is wrong:** the comment misreads what an initial `prev` means. It does
not assert "both logs are empty". It says "there is no preceding entry" — the
empty prefix. Every log contains the empty prefix, empty or not.

The spec condition is:

> Reply false if log doesn't contain an entry at prevLogIndex whose term matches
> prevLogTerm (§5.3)

At `prevLogIndex = 0` there is no entry to look up, so there is nothing to fail.
The check is vacuously satisfied. A Follower must accept.

**Failure walkthrough:**

1. Follower F holds `[ (term 1, cmd 3), (term 1, cmd 6) ]`.
2. Leader L is elected for term 2. L's log is shorter — legal, because
   `is_candidate_log_up_to_date` (`src/log.rs:138-156`) compares only the *last*
   `(term, idx)`. A higher last-term with fewer entries wins.
3. L sends AppendEntries with `prev` at some idx; F rejects; L decrements
   `next_idx` (`src/mode/leader.rs:276-284`).
4. `next_idx` walks down to 1. `prev` is now the initial `TermIdx`.
5. F still has a non-empty log, so `entry_matches` returns `DoesntExist` and F
   replies false again.
6. L has nothing smaller to try. F's stale term-1 entries can never be
   truncated. The peer is stranded permanently.

Step 5 also feeds bug 3: the false reply at `next_idx == 1` drives `next_idx` to
`Idx(0)`.

**Fix:** drop the log-empty condition.

```rust
// TermIdx::initial names the empty prefix preceding the first entry. Every log
// contains that prefix, so it matches regardless of what this log holds.
if term_idx.is_initial() {
    return MatchOutcome::Match;
}
```

Nothing else is needed. Once the Follower accepts, the append loop
(`src/mode/follower.rs:97-103`) calls `update_to_match_leaders_log` per entry,
and the `NoMatch` arm (`src/log.rs:81-91`) truncates the conflicting tail before
pushing:

```rust
outcome @ MatchOutcome::NoMatch => {
    self.entries.truncate(entry_idx.as_log_idx());
    self.entries.push(entry);
```

**Tests to update:** `test_entry_matches` (`src/log.rs:263-279`) currently pins
the buggy behavior:

```rust
// Non-empty log
log.push(vec![entry.clone()]);
assert!(matches!(
    log.entry_matches(TermIdx::initial()),
    MatchOutcome::DoesntExist   // becomes MatchOutcome::Match
));
```

**Test to add** (in `src/mode/follower.rs` tests, modeled on
`test_recv_append_entries`): a Follower holding two term-1 entries receives
`prev = TermIdx::initial()`, `leader_commit_idx = Idx::initial()`, and one
term-2 entry. Assert the response is `success: true`, `log.test_len() == 1`, and
`log.test_get_unchecked(1) == Entry::new(term 2, ..)` — the conflicting entry is
replaced and the tail dropped. Today it replies false and leaves the log alone.

### 2. follower: asserts on a peer-supplied value

**Where:** `src/mode/follower.rs:105-115`

```rust
//% Compliance:
//% If leaderCommit > commitIndex, set commitIndex = min(leaderCommit, index of
//% last new entry)
assert!(
    leader_commit_idx <= &raft_state.log.last_idx(),
    "leader_commit_idx should not be greater than the number of enties in the log"
);
if leader_commit_idx > raft_state.commit_idx() {
    let min_idx = min(*leader_commit_idx, raft_state.log.last_idx());
    raft_state.set_commit_idx(min_idx, peer_id, CurrentMode::Follower);
}
```

**Why it is wrong:** the assert and the `min` on the next line contradict each
other. `min(leaderCommit, last_idx)` clamps `leaderCommit` down to the log
length — that clamp only ever does anything when `leaderCommit > last_idx`,
which is exactly what the assert forbids. Either the assert is unreachable and
the `min` is the real logic, or the assert fires on the case the spec tells you
to handle. Both cannot be true.

The spec chose the `min` deliberately. A Leader's `commitIndex` routinely
exceeds a lagging Follower's log length; that is the normal state of a Follower
catching up.

**Why it does not fire today:** the Leader always sends its entire tail, so a
successful append leaves the Follower at exactly the Leader's `last_idx`, which
is `>= leader_commit_idx`. Two things break that:

- Batching. Ship 10 entries at a time and a Follower that is 50 behind ends the
  RPC well short of `leader_commit_idx`. Panic.
- A stale or duplicated RPC arriving after the log was truncated by a later
  Leader.

**The bigger problem:** `leader_commit_idx` is decoded straight off the wire
(`src/packet/append_entries.rs:74-96`). This is a remotely triggerable panic —
one malformed packet kills the node. Never assert on peer input. Validate and
reject, or clamp.

**Fix:** delete the assert. The `min` already handles it, and
`set_commit_idx` is only reached when `leader_commit_idx > commit_idx`.

### 3. leader: `next_idx` underflow

**Where:** `src/mode/leader.rs:276-284`

```rust
//% Compliance:
//% If AppendEntries fails because of log inconsistency: decrement nextIndex and retry (§5.3)
self.next_idx.entry(peer_id).and_modify(|idx| {
    assert!(
        !idx.is_initial(),
        "Peer responded false to initial Idx, which is malformed behavior."
    );
    *idx = idx.sub(1)
});
```

**Why it is wrong:** the assert checks the value *before* the subtraction. It
catches `next_idx == 0`, which cannot happen anyway, and permits `next_idx == 1`
— the one case that actually breaks. `1.sub(1)` is `Idx(0)`.

`Sub` itself will not save you (`src/log/idx.rs:36-44`):

```rust
fn sub(self, rhs: u64) -> Self::Output {
    debug_assert!(self.0 > 0, "value overflowed on subtraction");
    let idx_sub_one = self.0.saturating_sub(rhs);
    Idx(idx_sub_one)
}
```

`self.0 > 0` holds for 1, so the `debug_assert` passes and the result is 0.

**Why 1 is the floor:** `next_idx == 1` produces `prev = initial`, the empty
prefix. Every log matches it (after bug 1 is fixed), so there is nothing left to
retry with. Backing off further is meaningless.

**What `Idx(0)` corrupts downstream:**

- `get_entries` (`src/log.rs:26-35`) — `debug_assert!(!idx.is_initial())` fires.
- `as_log_idx()` (`src/log/idx.rs:21-25`) — `0 - 1` underflows.
- `term_at_idx` (`src/log.rs:60`) — `assert!(!idx.is_initial())` fires.

In release, `as_log_idx()` wraps to `usize::MAX`, `Vec::get` returns `None`, and
the failure turns silent instead of loud.

**Reachability:** bug 1 is the trigger. A Follower with a divergent non-empty
log rejects at `next_idx == 1`, and this branch runs.

**Fix:** floor the decrement at 1.

```rust
self.next_idx.entry(peer_id).and_modify(|idx| {
    assert!(
        *idx > Idx::from(1),
        "Peer rejected an initial prev TermIdx, which every log matches."
    );
    *idx = idx.sub(1)
});
```

Note the assert now describes a real protocol violation: a peer that rejects the
empty prefix is misbehaving.

### 4. leader: `prev_log_term_idx` is built at `next_idx`

**Where:** `src/mode/leader.rs:105-124`

```rust
let last_log_term_idx = raft_state.log.last_term_idx();
let peer_next_idx = *self
    .next_idx
    .get(peer_id)
    .expect("peer should have next_idx state");

//% Compliance:
//% If last log index ≥ nextIndex for a follower: send AppendEntries RPC with log
//% entries starting at nextIndex
let peer_term_idx = if last_log_term_idx.idx >= peer_next_idx {
    let peer_next_term = raft_state.log.term_at_idx(&peer_next_idx).unwrap();
    TermIdx::builder()
        .with_term(peer_next_term)
        .with_idx(peer_next_idx)
} else {
    // The peer should be theoritically up-to-date so send the latest Leader entry. If
    // the peer is not up-to-date we will get a failure and will decrement the peer's
    // nextIndex.
    last_log_term_idx
};
```

**Why it is wrong:** `peer_term_idx` goes into the RPC's `prev_log_term_idx`
field (`src/mode/leader.rs:126-132`). That field means "the entry preceding the
new ones" — `next_idx - 1`. This code puts `next_idx` itself there.

Two definitions collide on one number:

- `next_idx` = the entry the peer does **not** have yet (`src/mode/leader.rs:16-17`).
- `prev` = an entry the peer **must already** have, or it rejects the RPC.

**Why it has not blown up:** `entries` is always empty (bug 5). The Follower's
append loop never runs, so `prev.idx + 1` is never used. The only live effect is
the match check, which happens to work as a crude "are you caught up" probe.

**Failure walkthrough, once entries are sent:**

1. Leader log: `[e1, e2, e3]`. Peer has `[e1]`, so `next_idx == 2`.
2. Leader sends `prev.idx = 2` (this code) and `entries = [e2, e3]`.
3. Follower checks `entry_matches` at idx 2 — it does not have idx 2 →
   `DoesntExist` → replies false. Replication stalls one index short forever.
4. Suppose instead the peer *does* have idx 2 and the check passes. The Follower
   computes `entry_idx = prev.idx + 1 = 3` (`src/mode/follower.rs:97`) and
   writes `e2` at index 3, `e3` at index 4. Entry 2 is skipped. Every entry is
   stored one slot too high, under the wrong index, with the wrong term
   association. The `DoesntExist` arm's monotonicity assert
   (`src/log.rs:92-99`) is the likeliest thing to fire first.

That is silent log divergence — the failure mode Raft exists to prevent.

**Fix:** build `prev` from `next_idx - 1`. The `if/else` is no longer needed;
the "peer is up to date" case falls out naturally, because `get_entries` returns
empty when `next_idx == last_idx + 1`.

```rust
//% Compliance:
//% prevLogIndex: index of log entry immediately preceding new ones
//% prevLogTerm: term of prevLogIndex entry
let prev_idx = peer_next_idx - 1;
let prev_log_term_idx = if prev_idx.is_initial() {
    // The peer has nothing, so the entries are preceded by the empty prefix.
    TermIdx::initial()
} else {
    let prev_term = raft_state
        .log
        .term_at_idx(&prev_idx)
        .expect("prev_idx is <= last_idx so the entry exists");
    TermIdx::builder().with_term(prev_term).with_idx(prev_idx)
};
```

`term_at_idx` is safe here: `next_idx <= last_idx + 1`, so `prev_idx <=
last_idx`.

`last_log_term_idx` (`src/mode/leader.rs:105`) becomes unused — remove it.

**Tests to update:** `on_leader_with_entries` (`src/mode/leader.rs:347`) and
`on_timeout_uses_peer_next_idx` (`src/mode/leader.rs:447`) both assert the old
`prev` values. For a peer at `next_idx == 3` with a 2-entry log, `prev` becomes
`{term, idx 2}` and `entries` is empty. For a peer at `next_idx == 1`, `prev`
becomes `TermIdx::initial()` and `entries` is the whole log.

### 5. leader: no entries are ever sent

**Where:** `src/mode/leader.rs:126-134`

```rust
let rpc = Rpc::new_append_entry(
    leader_current_term,
    *server_id,
    peer_term_idx,
    leader_commit_idx,
    vec![],
);

peer_id.send_rpc(rpc, io_egress);
```

**Why it is wrong:** every AppendEntries is a heartbeat. No entry ever reaches a
Follower, so no entry is ever replicated, no `match_idx` ever advances, and
nothing is ever committed. The compliance comment directly above
(`src/mode/leader.rs:111-113`) says the opposite:

> If last log index ≥ nextIndex for a follower: send AppendEntries RPC with log
> entries starting at nextIndex

**Fix:**

```rust
// log[next_idx ..= last_idx]. Empty when the peer is already up-to-date, which
// makes this RPC a heartbeat.
let entries = raft_state.log.get_entries(&peer_next_idx);
```

and pass `entries` instead of `vec![]`.

`get_entries` (`src/log.rs:26-35`) already handles the boundary: `next_idx ==
last_idx + 1` slices at `len` and yields an empty `Vec`. Its two `debug_assert`s
bound the legal input to `[1, last_idx + 1]`, exactly the range of `next_idx`.

**Depends on bug 4.** With the current `prev` convention, entries land at
`prev.idx + 1 == next_idx + 1` on the Follower — off by one. Do not ship entries
until `prev` is `next_idx - 1`.

### 6. leader: response matching uses the wrong index

**Was:** `src/mode/leader.rs:258-265` and `:302-305`

```rust
let current_next_idx = *self
    .next_idx
    .get(&peer_id)
    .expect("peer should have next_idx state");

// Only process the response if the RPC matches the current next_idx for the peer. The RPC
// can be out-of-order due to timeout and re-transmission.
if current_next_idx.eq(&echo_prev_log_term_idx.idx) {
```

```rust
} else {
    // RPC was received out of order and didn't match the peer's next_idx.
    None
}
```

**What the echo is:** the Follower copies the request's `prev_log_term_idx`
into the response verbatim (`src/mode/follower.rs:115`):

```rust
let rpc = Rpc::new_append_entry_resp(current_term, response, *prev_log_term_idx);
```

So `echo_prev_log_term_idx` is whatever the Leader put in `prev`. The de-dup
check is sound in principle — it discards responses to superseded RPCs — but it
compares the echo against the wrong local value.

**Why it was wrong:** it only held while bug 4 made `prev == next_idx`. Once
that was fixed the echo became `next_idx - 1` and this comparison never matched.
Every response fell to the `else` and was silently discarded — the Leader could
not advance, commit, or retry a failure, with no logging to show why.

**Fix:** compare against the value actually sent
(`src/mode/leader.rs:258-274`).

```rust
// The RPC echoes the prev TermIdx that was sent, which is next_idx - 1.
let expected_echo_prev_idx = {
    let current_next_idx = *self
        .next_idx
        .get(&peer_id)
        .expect("peer should have next_idx state");

    if current_next_idx.is_initial() {
        Idx::initial()
    } else {
        current_next_idx - 1
    }
};
```

The `is_initial` arm is belt-and-braces: bug 3's floor keeps `next_idx >= 1`, so
the subtraction is already safe.

**Tests:** `test_on_recv_append_entry_resp`
(`src/mode/leader.rs:564`) builds echoes at `Idx::from(3)` for a peer at
`next_idx == 3`. Those become `Idx::from(2)`, and the "out of order" case needs
a different value to stay out of order.

`on_recv_append_entry_resp_does_not_decrement_next_idx_past_one`
(`src/mode/leader.rs:852`) had to switch its echo to `Idx::initial()`, or the
response is discarded before reaching the decrement and the test stops
exercising the assert.

`on_recv_append_entry_resp_matches_on_echoed_prev_idx`
(`src/mode/leader.rs:671`) covers this directly: a stale echo leaves `next_idx`
alone, an echo of `next_idx - 1` is processed.

### 7. leader: success does not advance `next_idx` / `match_idx`

**Was:** `src/mode/leader.rs:266-282`

```rust
if *success {
    // Check the TermIdx in the Resp rpc rather than assuming next_idx to make the
    // protocol more resilient.
    let rpc_sent_idx = *self
        .next_idx
        .get(&peer_id)
        .expect("peer should have next_idx state");

    //% Compliance:
    //% If successful: update nextIndex and matchIndex for follower (§5.3)
    self.next_idx
        .entry(peer_id)
        .and_modify(|idx| *idx = rpc_sent_idx);
    self.match_idx
        .entry(peer_id)
        .and_modify(|idx| *idx = rpc_sent_idx);
    Some(rpc_sent_idx)
```

**Why it was wrong:** `rpc_sent_idx` was read out of `next_idx`, then written
back into `next_idx`. A no-op. The comment claimed it read the response's TermIdx
"rather than assuming next_idx" — but it never touched `echo_prev_log_term_idx`;
it did exactly what it said it avoided.

Consequences:

- `next_idx` never moved. A peer that accepted an RPC was asked for the same
  index on every subsequent heartbeat, forever.
- `match_idx` was set to `next_idx`, an entry the peer is not known to hold.
  Under the old `prev == next_idx` convention that was coincidentally the matched
  entry; after bug 4 it would have been one too high, letting `update_commit_idx`
  commit an entry a quorum does not have. A safety violation, not just a stall.

**The missing information:** `match_idx` is `prev.idx + entries.len()`. The
response carried `prev.idx` but not the count, and the Leader could not recover
it — by the time a response lands `next_idx` may have moved, and `log.last_idx()`
only works as a stand-in because a Leader's log never grows today. That stops
being true with the first client write.

**Fix — the response reports the count.** `AppendEntriesResp` gained
`entries_cnt` (`src/packet/append_entries.rs:69-78`):

```rust
// The number of entries stored from the [AppendEntries] RPC.
//
// The Follower accepts all of the entries or none of them, so this is the RPC's entries.len()
// on success and 0 on failure.
pub entries_cnt: EntriesLenTypeEncoding,
```

A count rather than an absolute Idx, for two reasons. The Follower is
all-or-nothing (`src/mode/follower.rs:90-112`) — it either rejects the RPC or
runs the append loop to completion — so the count is exactly what was stored.
And it is *anchored*: the Leader computes `match_idx` from the echoed prev, a
TermIdx it has already matched against its own `next_idx`, plus the count. An
absolute Idx would be unanchored and could contradict the echo.

Leader side (`src/mode/leader.rs:275-299`):

```rust
let peer_match_idx = min(
    echo_prev_log_term_idx.idx + *entries_cnt as u64,
    raft_state.log.last_idx(),
);

self.next_idx
    .entry(peer_id)
    .and_modify(|idx| *idx = peer_match_idx + 1);
self.match_idx
    .entry(peer_id)
    .and_modify(|idx| *idx = peer_match_idx);

(!peer_match_idx.is_initial()).then_some(peer_match_idx)
```

The `min` is load-bearing, not defensive dressing: `entries_cnt` arrives off the
wire and `match_idx` feeds `update_commit_idx`, so an inflated count would commit
entries no quorum holds.

A heartbeat ack falls out correctly — `entries_cnt == 0` gives
`match_idx = prev.idx`, exactly what the peer proved it holds.

The `is_initial` guard avoids the `term_at_idx` assert in `update_commit_idx`
(`src/mode/leader.rs:225-231`), which panics on an initial `Idx`. A peer
acknowledging an empty log produces exactly that.

**Also touched:** the Follower reports the count
(`src/mode/follower.rs:114-123`), the Candidate's rejection path passes 0
(`src/mode/candidate.rs:113`), `Rpc::new_append_entry_resp` takes a fourth
argument (`src/packet/rpc.rs:52-64`), and the resp encode/decode carries the
field (`src/packet/append_entries.rs:133-158`).

**Tests:** `on_recv_append_entry_resp_success_advances_next_and_match_idx`
(`src/mode/leader.rs:758`) covers both shapes — a peer sent the whole log behind
an empty prefix, and a caught-up peer acking a bare heartbeat. It asserts the
resulting `match_idx` and `next_idx` rather than the mechanism, so it holds
regardless of how the count reaches the Leader.

**Monotonicity:** `match_idx` is also floored at its current value
(`src/mode/leader.rs:289-301`):

```rust
let peer_match_idx = max(reported_match_idx, current_match_idx);
```

The echo check does not make this redundant. It compares against `next_idx - 1`,
and `next_idx` is walked backwards on every failure, so a delayed success can
arrive once `next_idx` has dropped to the value that response was sent under.
Replication is append-only — an entry known to be stored stays stored — so
letting `match_idx` fall would drop an entry back below the quorum
`update_commit_idx` counts.

Covered by `on_recv_append_entry_resp_match_idx_never_moves_backward`
(`src/mode/leader.rs:863`).
