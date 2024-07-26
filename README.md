# raft-rs

A toy implementation to better understand the
[Raft](https://toidiu.com/reads/In_Search_of_an_Understandable_Consensus_Algorithm_(Extended_Raft).pdf)
consensus protocol

## Tasks
Remaining tasks to complete:
- [x] IO
- [x] implement leader election
- [ ] Amend Entry to contain (idx, term, data)
- [ ] implement heartbeats
- [ ] implement AppendEntries
- [ ] implement Leader
- [ ] remove ServerId from RequestVote
- [ ] implement state machine storage

## Fig 2 compliance

**State**
#### Persistent state on all servers:
- [ ] "Updated on stable storage before responding to RPCs"
- [x] currentTerm latest term server has seen (initialized to 0 on first boot,
  increases monotonically)
- [x] votedFor candidateId that received vote in current term (or null if none)
- [x] log[] log entries; each entry contains command for state machine, and term
  when entry was received by leader (first index is 1)
#### Volatile state on all servers:
- [ ] commitIndex index of highest log entry known to be committed (initialized
  to 0, increases monotonically)
- [ ] lastApplied index of highest log entry applied to state machine
  (initialized to 0, increases monotonically)
#### Volatile state on leaders:
- [ ] "Reinitialized after election"
- [ ] nextIndex[] for each server, index of the next log entry to send to that
  server (initialized to leader last log index + 1)
- [ ] matchIndex[] for each server, index of highest log entry known to be
  replicated on server (initialized to 0, increases monotonically)

**AppendEntries**
#### Arguments:
- [x] term leader’s term
- [ ] leaderId so follower can redirect clients
- [x] prevLogIndex index of log entry immediately preceding new ones
- [x] prevLogTerm term of prevLogIndex entry
- [ ] entries[] log entries to store (empty for heartbeat; may send more than
  one for efficiency)
- [ ] leaderCommit leader’s commitIndex
#### Results:
- [x] term currentTerm, for leader to update itself
- [x] success true if follower contained entry matching prevLogIndex and
  prevLogTerm
#### Receiver implementation:
- [x] Reply false if term < currentTerm (§5.1)
- [x] Reply false if log doesn’t contain an entry at prevLogIndex whose term
  matches prevLogTerm (§5.3)
- [ ] If an existing entry conflicts with a new one (same index but different
  terms), delete the existing entry and all that follow it (§5.3)
- [ ] Append any new entries not already in the log
- [ ] If leaderCommit > commitIndex, set commitIndex = min(leaderCommit, index
  of last new entry)

**RequestVote**
#### Arguments:
- [x] term candidate’s term
- [x] candidateId candidate requesting vote
- [x] lastLogIndex index of candidate’s last log entry (§5.4)
- [x] lastLogTerm term of candidate’s last log entry (§5.4)
#### Results:
- [x] term currentTerm, for candidate to update itself
- [x] voteGranted true means candidate received vote
#### Receiver implementation:
- [x] Reply false if term < currentTerm (§5.1)
- [x] If candidate’s log is at least as up-to-date as receiver’s log
    - [x] and votedFor is null, grant vote (§5.2, §5.4)
    - [x] and votedFor is candidateId, grant vote (§5.2, §5.4)

### Rules for Servers
**All Servers:**
- [ ] If commitIndex > lastApplied: increment lastApplied
- [ ] If commitIndex > lastApplied: apply log[lastApplied] to state machine (§5.3)
- [x] If RPC request or response contains term T > currentTerm: set currentTerm
  = T, convert to follower (§5.1)

**Followers (§5.2):**
- [x] Respond to RPCs from candidates and leaders
- [x] If election timeout elapses without receiving AppendEntries RPC from
  current leader or granting vote to candidate: convert to candidate

**Candidates (§5.2):**
- [x] On conversion to candidate, start election:
  - [x] Increment currentTerm
  - [x] Vote for self
  - [x] Reset election timer
  - [x] Send RequestVote RPCs to all other servers
- [x] If votes received from majority of servers: become leader
- [x] If AppendEntries RPC received from new leader: convert to follower
- [x] If election timeout elapses: start new election

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


## Research

### Design
![io_design](./io_design.jpeg)
![raft_design](./raft_design.jpeg)

### Notes from [TIKV](https://github.com/tikv/raft-rs)

> A complete Raft model contains 4 essential parts:
>
> - Consensus Module, the core consensus algorithm module;
> - Log, the place to keep the Raft logs;
> - State Machine, the place to save the user data;
> - Transport, the network layer for communication.

`struct StateMachine`

"Replicated state machine". The concept that the same data is spread over many
machines so the failure of minority of machines doesnt impact liveliness.

`struct ReplicatedLog`

Replicated state machines are implemented via a "replicated log", which are a serive of
commands to be executed in-order.

`struct ConsensusAlgo`

The job of the "consensus algorithm" is keeping the replicated log consistent.

The consensus algo on a server receives commands from a client and adds them it its
log. It then commicumates with other consensus algo on other servers to ensure that
every log contains the same commands in the same order.

3 properties
- Leader Election
- Log Repliation: leader manages replicated log
- Safety: entries added to the state machine are absolute and correct

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

