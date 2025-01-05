## Citations

# Requirements

## Fig 2: condensed summary of Raft

### State
#### Persistent state on all servers
- [ ] Updated on stable storage before responding to RPCs
- [x] `currentTerm` latest term server has seen (initialized to 0 on first boot, increases monotonically)
- [x] `votedFor` `candidateId` that received vote in current term (or null if none)
- [x] `log[]` log entries; each entry contains command for state machine, and term when entry was received by leader (first index is 1)
#### Volatile state on all servers
- [x] `commitIndex` index of highest log entry known to be committed (initialized to 0, increases monotonically)
- [x] `lastApplied` index of highest log entry applied to state machine (initialized to 0, increases monotonically)
#### Volatile state on leaders
- [ ] "Reinitialized after election"
- [x] `nextIndex[]` for each server, index of the next log entry to send to that server (initialized to leader last log index + 1)
- [x] `matchIndex[]` for each server, index of highest log entry known to be replicated on server (initialized to 0, increases monotonically)

### Rules for Servers
#### All Servers
- [ ] If commitIndex > lastApplied: increment lastApplied
- [ ] If commitIndex > lastApplied: apply log[lastApplied] to state machine (§5.3)
- [x] If RPC request or response contains term T > currentTerm: set currentTerm = T, convert to follower (§5.1)
#### Followers (§5.2)
- [ ] Respond to RPCs from candidates and leaders
- [x] If election timeout elapses without receiving AppendEntries RPC from current leader or granting vote to candidate: convert to candidate
#### Candidates (§5.2)
- [x] On conversion to candidate, start election:
  - [ ] Increment currentTerm
  - [ ] Vote for self
  - [ ] Reset election timer
  - [ ] Send RequestVote RPCs to all other servers
- [ ] If votes received from majority of servers: become leader
- [ ] If AppendEntries RPC received from new leader: convert to follower
- [ ] If election timeout elapses: start new election
#### Leaders
- [ ] Upon election: send initial empty AppendEntries RPCs (heartbeat) to each server; repeat during idle periods to prevent election timeouts (§5.2)
- [ ] If command received from client: append entry to local log, respond after entry applied to state machine (§5.3)
- [ ] If last log index ≥ nextIndex for a follower: send AppendEntries RPC with log entries starting at nextIndex
  - [ ] If successful: update nextIndex and matchIndex for follower (§5.3)
  - [ ] If AppendEntries fails because of log inconsistency: decrement nextIndex and retry (§5.3)
- [ ] If there exists an N such that N > commitIndex, a majority of matchIndex[i] ≥ N, and log[N].term == currentTerm: set commitIndex = N (§5.3, §5.4).

### AppendEntries RPC
#### Arguments
- [x] term: leader’s term
- [x] leaderId: so follower can redirect clients
- [x] prevLogIndex: index of log entry immediately preceding new ones
- [x] prevLogTerm: term of prevLogIndex entry
- [x] entries[]: log entries to store (empty for heartbeat; may send more than one for efficiency)
- [x] leaderCommit: leader’s commitIndex
#### Results
- [x] term: currentTerm, for leader to update itself
- [x] success: true if follower contained entry matching prevLogIndex and
  prevLogTerm
#### Receiver implementation
- [ ] Reply false if term < currentTerm (§5.1)
- [ ] Reply false if log doesn’t contain an entry at prevLogIndex whose term  matches prevLogTerm (§5.3)
- [ ] If an existing entry conflicts with a new one (same index but different terms), delete the existing entry and all that follow it (§5.3)
- [ ] Append any new entries not already in the log
- [ ] If leaderCommit > commitIndex, set commitIndex = min(leaderCommit, index of last new entry)

### RequestVote RPC
#### Arguments
- [x] term: candidate’s term
- [x] candidateId: candidate requesting vote
- [x] lastLogIndex: index of candidate’s last log entry (§5.4)
- [x] lastLogTerm: term of candidate’s last log entry (§5.4)
#### Results
- [x] term: currentTerm, for candidate to update itself
- [x] voteGranted: true means candidate received vote
#### Receiver implementation
- [ ] Reply false if term < currentTerm (§5.1)
- [ ] If candidate’s log is at least as up-to-date as receiver’s log
    - [ ] and votedFor is null, grant vote (§5.2, §5.4)
    - [ ] and votedFor is candidateId, grant vote (§5.2, §5.4)

## 5: The Raft consensus algo
- [ ] first elect a leader
- [ ] leader then responsible for managing the replicated log
- [ ] leader accepts log entries from clients
	- [ ] replicates them on other servers
	- [ ] tells other servers when its safe to apply log entries to state machine
**Raft sub-problem:**
- **Leader election:** choose a new leader when the existing one fails
- **Log replication:** leader accepts log entries from client and replicate them across cluster, forcing other logs to agree with its own
- **Safety:** (State Machine Safety Property). If a server has applied a log entry to state machine, then no other server will apply a different entry to the same log index

### 5.1 Raft basics
- [ ] server can be in one of 3 modes: leader, follower candidate
	- [ ] normal operation: 1 leader and other are followers
	- [ ] followers are passive and only respond to requests
	- [ ] leader handles all client requests
	- [ ] candidate state is used to elect a new leader
- [ ] the unit of time is `terms`: logical clock
	- [ ] which are consecutive integers
	- [ ] begins with a new election
	- [ ] there is at most one leader per term
	- [ ] if an election ends with a split vote then the term ends with no leader
- [ ] each server stores the `current term`
	- [ ] increases monotonically
	- [ ] if a server sees a larger term, it updates its `current term`
	- [ ] if a candidate or leader sees a larger term, it reverts to follower state
	- [ ] a request with a stale term is rejected
- [ ] RPCs
	- [ ] issued in parallel for best performance
	- [ ] retried if no response is received
	- [ ] RequestVote
		- [ ] initiated by a candidate during election
	- [ ] AppendEntries
		- [ ] initiated by a leader
		- [ ] used to replicate log entries
		- [ ] also a heartbeat mechanism
	- [ ] InstallSnapshot (optional)

### 5.2 Leader Election
- [ ] heartbeat are used to trigger election
- [ ] servers begin as followers
	- [ ]  a server remains a follower as long as it receives a valid RPC from a leader or candidate
- [ ] leaders send periodic heartbeat
- [ ] heartbeat is a AppendEntries RPC with no log entries
- [ ] a follower that receives no communication (election timeout) assumes there is no viable leader
	- [ ] increments its current term
	- [ ] transitions to `candidate`
	- [ ] votes for itself
	- [ ] issues a RequestVote in parallel to other servers
- [ ] The `candidate` continues in the above state until one of:
	- [ ] wins election
		- [ ] receives majority of votes in cluster (ensures a single winner)
		- [ ] a server can only vote once for a given term (first-come basis)
		- [ ] a candidate becomes `leader` if it wins the election
		- [ ] sends a heartbeat to establish itself as a leader and prevent a new election
	- [ ] another server establishes itself as a leader
		- [ ] a candidate receives AppendEntries from another server claiming to be a leader
		- [ ] if that leader's current term is >= the candidate's
			- [ ] recognize the server as the new leader
			- [ ] then the candidate reverts to a follower
		- [ ] if the leader's current term is < the candidate's
			- [ ] reject the RPC and continue in the candidate state
	- [ ] a timeout occurs and there is no winner (can happen if too many servers become candidates at the same time)
		- [ ] increment its term
		- [ ] start a new election by initiating another round of RequestVote
- [x] Election timeout is chosen randomly between 150-300ms

### 5.3 Log replication
- [ ] a leader services client requests
	- each request contains a command to be executed by the state machine
	- [ ] the leader appends the command to its log as a new entry
	- [ ] issues AppendEntries in parallel to replicate the entry
	- after the entry has been safely replicated
		- [ ] the leader applies the entry to its state machine
	- [ ] leader indefinitely reties AppendEntries in the face of packet loss/network issues
- [x] each log entry stores
	- [x] a state machine command
	- [x] term number
	- [x] log index: integer
- [ ] leader decides when to `commit` an entry
	- `commit`: when its safe to apply an entry to the state machine
	- `apply` entry to state machine: actually write an entry to the state machine
	- [ ] An entry is `committed` when the leader that created the entry, replicates it on majority of servers
		- this also commits all preceding entries in the leader's log
	- [ ] leader tracks the highest index it knows to be committed
		- [ ] includes that number in future AppendEntries
		- [ ] once a follower learns an entry is committed (`leaderCommit` in AppendEntries), it applies the entry to its state machine
- **Log Matching Property**
	- if two entries in different logs have the same index/term, they store the same command
	- if two entries in different logs have the same index/term, all preceding entries are identical
- AppendEntries helps perform a consistency check
	- [ ] the leader includes the index and term of the entry immediately preceding the new entries
		- [ ] if the follower doesn't find an entry with the same index and term, it refuses the new entries
- leader handles inconsistent logs by forcing follower to duplicate its own logs. conflicting follower logs are overwritten with the leader's logs
	- to make the leader/follower logs consistent:
		- [ ] the leader finds the last log entry that are the same
		- [ ] follower deletes any entries after that point
		- [ ] leader sends the follower all its entries after the common point
	- [ ] leader maintains a `nextIndex` for each follower: index of the next log entry the leader will send to that follower
	- [ ] when a server first becomes a leader
		- [ ] initializes `nextIndex` to +1 of the last entry in its log
	- [ ] if a follower's log is inconsistent with the leader's, AppendEntries RPC will fail
		- [ ] after a failed AppendEntries, the leader will decrement `nextIndex` and retry AppendEntries
			- [ ] Eventually `nextIndex` will match the follower's and leader's logs
			- [ ] after logs match
				- [ ] AppendEntries will succeed
				- [ ] follower removes conflicting entries from its logs
				- [ ] follower appends entries from the leader
			- [ ] (optional) its possible to optimize finding the matching `nextIndex` between leader and follower
- [ ] A leader never deletes of overwrites its own logs

### 5.4 Safety
- restrict which servers can be elected leaders
	- [ ] leader for any term must contain all entries committed in previous terms

#### 5.4.1 Election restriction
- All committed entries from previous terms are present on each new leader when its elected.
	- [ ] log entries only flow from leader to follower.
	- [ ] leader never overwrites existing entries in its log.
- [ ] a candidate cant win an election unless its log contains all committed entries.
	- [ ] a candidate must get a vote from a majority of servers
	- `up-to-date`: a log is considered more up-to-date than another log if:
		- [ ] compare the index and term of the last entry of A's and B's log
		- [ ] if the entries have different term: the higher term is more up-to-date
		- [ ] if the term is the same: the longer log (higher index) is more up-to-date
	- The RequestVote RPC helps ensure the leader's log is `up-to-date`
		- [ ] RequestVote includes info about candidate's log
		- [ ] voter denies vote if its own log is more `up-to-date`

#### 5.4.2 Committing entries from previous terms
- [ ] a leader knows an entry from its **current term** (not true for previous terms) is committed, once its stored (replicated) on a majority of servers
	- [ ] an entry is replicated if the server responds with success to AppendEntries
- [ ] a leader can NOT conclude an entry from a previous term is committed once it is stored on a majority of servers (a previous entry could be overwritten)
	- log entries retain their original term number even when a leader entries previous terms. This makes logs easier to reason about
	- [ ] never commit entries from previous terms by counting replicas
	- [ ] only entries from the leader's current term are committed by counting replicas
	- [ ] once an entry from the current term is committed, previous entries will indirectly be committed

#### 5.4.3 Safety argument
- [ ] entries are applied/committed in log index order
	- this (combined with State Machine Safety Property) ensures that all servers apply the same set of log entries to state machine, in the same order

