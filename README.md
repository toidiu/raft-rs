# raft-rs

A toy implementation to better understand the
[Raft](https://toidiu.com/reads/In_Search_of_an_Understandable_Consensus_Algorithm_(Extended_Raft).pdf)
consensus protocol

## Fig 2: condensensed summary of Raft

### State
**Persistent state on all servers:**
- [ ] "Updated on stable storage before responding to RPCs"
- [ ] `currentTerm` latest term server has seen (initialized to 0 on first boot,
  increases monotonically)
- [ ] `votedFor` candidateId that received vote in current term (or null if
  none)
- [ ] `log[]` log entries; each entry contains command for state machine, and
  term when entry was received by leader (first index is 1)

**Volatile state on all servers:**
- [ ] `commitIndex` index of highest log entry known to be committed
  (initialized to 0, increases monotonically)
- [ ] `lastApplied` index of highest log entry applied to state machine
  (initialized to 0, increases monotonically)

**Volatile state on leaders:**
- [ ] "Reinitialized after election"
- [ ] `nextIndex[]` for each server, index of the next log entry to send to that
  server (initialized to leader last log index + 1)
- [ ] `matchIndex[]` for each server, index of highest log entry known to be
  replicated on server (initialized to 0, increases monotonically)

### Rules for Servers
**All Servers:**
- [ ] If commitIndex > lastApplied: increment lastApplied
- [ ] If commitIndex > lastApplied: apply log[lastApplied] to state machine (§5.3)
- [ ] If RPC request or response contains term T > currentTerm: set currentTerm
  = T, convert to follower (§5.1)

**Followers (§5.2):**
- [ ] Respond to RPCs from candidates and leaders
- [ ] If election timeout elapses without receiving AppendEntries RPC from
  current leader or granting vote to candidate: convert to candidate

**Candidates (§5.2):**
- [ ] On conversion to candidate, start election:
  - [ ] Increment currentTerm
  - [ ] Vote for self
  - [ ] Reset election timer
  - [ ] Send RequestVote RPCs to all other servers
- [ ] If votes received from majority of servers: become leader
- [ ] If AppendEntries RPC received from new leader: convert to follower
- [ ] If election timeout elapses: start new election

**Leaders:**
- [ ] Upon election: send initial empty AppendEntries RPCs (heartbeat) to each
  server; repeat during idle periods to prevent election timeouts (§5.2)
- [ ] If command received from client: append entry to local log, respond after
  entry applied to state machine (§5.3)
- [ ] If last log index ≥ nextIndex for a follower: send AppendEntries RPC with
  log entries starting at nextIndex
  - [ ] If successful: update nextIndex and matchIndex for follower (§5.3)
  - [ ] If AppendEntries fails because of log inconsistency: decrement nextIndex
    and retry (§5.3)
- [ ] If there exists an N such that N > commitIndex, a majority of
  matchIndex[i] ≥ N, and log[N].term == currentTerm: set commitIndex = N (§5.3,
  §5.4).

### AppendEntries RPC
**Arguments:**
- [ ] term leader’s term
- [ ] leaderId so follower can redirect clients
- [ ] prevLogIndex index of log entry immediately preceding new ones
- [ ] prevLogTerm term of prevLogIndex entry
- [ ] entries[] log entries to store (empty for heartbeat; may send more than
  one for efficiency)
- [ ] leaderCommit leader’s commitIndex

**Results:**
- [ ] term currentTerm, for leader to update itself
- [ ] success true if follower contained entry matching prevLogIndex and
  prevLogTerm

**Receiver implementation:**
- [ ] Reply false if term < currentTerm (§5.1)
- [ ] Reply false if log doesn’t contain an entry at prevLogIndex whose term
  matches prevLogTerm (§5.3)
- [ ] If an existing entry conflicts with a new one (same index but different
  terms), delete the existing entry and all that follow it (§5.3)
- [ ] Append any new entries not already in the log
- [ ] If leaderCommit > commitIndex, set commitIndex = min(leaderCommit, index
  of last new entry)

### RequestVote RPC
**Arguments:**
- [ ] term candidate’s term
- [ ] candidateId candidate requesting vote
- [ ] lastLogIndex index of candidate’s last log entry (§5.4)
- [ ] lastLogTerm term of candidate’s last log entry (§5.4)
**Results:**
- [ ] term currentTerm, for candidate to update itself
- [ ] voteGranted true means candidate received vote
**Receiver implementation:**
- [ ] Reply false if term < currentTerm (§5.1)
- [ ] If candidate’s log is at least as up-to-date as receiver’s log
    - [ ] and votedFor is null, grant vote (§5.2, §5.4)
    - [ ] and votedFor is candidateId, grant vote (§5.2, §5.4)

## Fig 3: Raft guarantees
- Election Safety: at most one leader can be elected in a given term. §5.2
- Leader Append-Only: a leader never overwrites or deletes entries in its log; it only appends new entries. §5.3
- Log Matching: if two logs contain an entry with the same index and term, then the logs are identical in all entries up through the given index. §5.3
- Leader Completeness: if a log entry is committed in a given term, then that entry will be present in the logs of the leaders for all higher-numbered terms. §5.4
- State Machine Safety: if a server has applied a log entry at a given index to its state machine, no other server will ever apply a different log entry for the same index. §5.4.3

## Design
**sans I/O design**
![io_queues](./queues.jpeg)

---
## Resources
- https://toidiu.com/reads/In_Search_of_an_Understandable_Consensus_Algorithm_(Extended_Raft).pdf
- https://web.stanford.edu/~ouster/cgi-bin/papers/OngaroPhD.pdf
- https://notes.eatonphil.com/2023-05-25-raft.html
- https://github.com/jmsadair/raft
- https://github.com/tikv/raft-rs
- https://notes.eatonphil.com/2023-05-25-raft.html
- https://raft.github.io/
- http://dabeaz.com/raft.html

