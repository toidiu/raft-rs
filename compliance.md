## Citations

### 5.2
- [ ] When servers start up, they begin as followers
- [ ] A server remains in fillower state as long as it receives valid RPCs from
  a leader or candidate.
- [ ] Leaders send periodic heartbeats (AppendEntries RPCs that carry no log
  entries) to all followers in order to maintain their authority.
- [ ] If a follower receives no communication over a period of time called the
  election timeout, then it assumes there is no viable leader and begins an
  election to choose a new leader.

- [ ] To begin an election, a follower increments its current term and
  transitions to candidate state.
- [ ] It then votes for itself and issues RequestVote RPCs in parallel to each
  of the other servers in the cluster.
- [ ] A candidate continues in this state until one of three things happens: (a)
  it wins the election, (b) another server establishes itself as leader, or (c)
  a period of time goes by with no winner.

- [ ] A candidate wins an election if it receives votes from a majority of the
  servers in the full cluster for the same term.
- [ ] Each server will vote for at most one candidate in a given term, on a
  first-come-first-served basis (note: Section 5.4 adds an additional
  restriction on votes).
- [ ] The majority rule ensures that at most one candidate can win the election
  for a particular term (the Election Safety Property in Figure 3).
- [ ] Once a candidate wins an election, it becomes leader.
- [ ] It then sends heartbeat messages to all of the other servers to establish
  its authority and prevent new elections.

- [ ] While waiting for votes, a candidate may receive an AppendEntries RPC from
  another server claiming to be leader.
- [ ] If the leader’s term (included in its RPC) is at least as large as the
  candidate’s current term, then the candidate recognizes the leader as
  legitimate and returns to follower state.
- [ ] If the term in the RPC is smaller than the candidate’s current term, then
  the candidate rejects the RPC and continues in candidate state.

- [ ] The third possible outcome is that a candidate neither wins nor loses the
  election: if many followers become candidates at the same time, votes could be
  split so that no candidate obtains a majority.
- [ ] When this happens, each candidate will time out and start a new election
  by incrementing its term and initiating another round of RequestVote RPCs.

- [ ] Raft uses randomized election timeouts to ensure that split votes are rare
  and that they are resolved quickly.
- [ ] To prevent split votes in the first place, election timeouts are chosen
  randomly from a fixed interval (e.g., 150–300ms).
- [ ] Each candidate restarts its randomized election timeout at the start of an
  election, and it waits for that timeout to elapse before starting the next
  election; this reduces the likelihood of another split vote in the new
  election.

### 5.3
- [ ] Once a leader has been elected, it begins servicing client requests.
- [ ] Each client request contains a command to be executed by the replicated
  state machines.
- [ ] The leader appends the command to its log as a new entry, then issues
  AppendEntries RPCs in parallel to each of the other servers to replicate the
  entry.
- [ ] When the entry has been safely replicated (as described below), the leader
  applies the entry to its state machine and returns the result of that
  execution to the client.
- [ ] If followers crash or run slowly, or if network packets are lost, the
  leader retries AppendEntries RPCs indefinitely (even after it has responded to
  the client) until all followers eventually store all log entries.

- [ ] Each log entry stores a state machine command along with the term number
  when the entry was received by the leader.
- [ ] Each log entry also has an integer index identifying its position in the
  log.

- [ ] The leader decides when it is safe to apply a log entry to the state
  machines; such an entry is called committed.
- [ ] Raft guarantees that committed entries are durable and will eventually be
  executed by all of the available state machines.
- [ ] A log entry is committed once the leader that created the entry has
  replicated it on a majority of the servers.
- [ ] This also commits all preceding entries in the leader’s log, including
  entries created by previous leaders.

- [ ] The leader keeps track of the highest index it knows to be committed, and
  it includes that index in future AppendEntries RPCs (including heartbeats) so
  that the other servers eventually find out.
- [ ] Once a follower learns that a log entry is committed, it applies the entry
  to its local state machine (in log order).


- [ ] Log Matching Property:
  - [ ] If two entries in different logs have the same index and term, then they
    store the same command.
  - [ ] If two entries in different logs have the same index and term, then the
    logs are identical in all preceding entries.

- [ ] conflicting entries in follower logs will be overwritten with entries from
  the leader’s log.
- [ ] To bring a follower’s log into consistency with its own, the leader must
  find the latest log entry where the two logs agree, delete any entries in the
  follower’s log after that point, and send the follower all of the leader’s
  entries after that point.
- [ ] All of these actions happen in response to the consistency check performed
  by AppendEntries RPCs.

- [ ] The leader maintains a nextIndex for each follower, which is the index of
  the next log entry the leader will send to that follower.
- [ ] When a leader first comes to power, it initializes all nextIndex values to
  the index just after the last one in its log.
- [ ] If a follower’s log is inconsistent with the leader’s, the AppendEntries
  consistency check will fail in the next AppendEntries RPC.
- [ ] After a rejection, the leader decrements nextIndex and retries the
  AppendEntries RPC.
- [ ] Eventually nextIndex will reach a point where the leader and follower logs
  match.
- [ ] When this happens, AppendEntries will succeed, which removes any
  conflicting entries in the follower’s log and appends entries from the
  leader’s log (if any).
- [ ] Once AppendEntries succeeds, the follower’s log is consistent with the
  leader’s, and it will remain that way for the rest of the term.
- [ ] A leader never overwrites or deletes entries in its own log.

### 5.4
- [ ] The restriction ensures that the leader for any given term contains all of
  the entries committed in previous terms.

### 5.4.1
- [ ]  This means that log entries only flow in one direction, from leaders to
  followers, and leaders never overwrite existing entries in their logs.
- [ ] The RequestVote RPC implements this restriction: the RPC includes
  information about the candidate’s log, and the voter denies its vote if its
  own log is more up-to-date than that of the candidate.
- [ ] Raft determines which of two logs is more up-to-date by comparing the
  index and term of the last entries in the logs.
- [ ] If the logs have last entries with different terms, then the log with the
  later term is more up-to-date.
- [ ] If the logs end with the same term, then whichever log is longer is more
  up-to-date.

### 5.4.2
- [ ]  Raft never commits log entries from previous terms by counting replicas.
  Only log entries from the leader’s current term are committed by counting
  replicas; once an entry from the current term has been committed in this way,
  then all prior entries are committed indirectly because of the Log Matching
  Property.

### Figure 6 An entry is considered committed if it is safe for that entry to be
applied to state machines.
