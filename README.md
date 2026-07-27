# raft-rs

A toy implementation to better understand the
[Raft](https://toidiu.com/reads/In_Search_of_an_Understandable_Consensus_Algorithm_(Extended_Raft).pdf)
consensus protocol

TODO:
- Add leader test
  - [ ] leader progression
  - [x] on_timeout
  - [x] update_commit_idx
  - [x] on_recv_append_entry_resp
- [ ] test: state machine tests
- [ ] Client API: on the server to propose a command (stores `u8` data)
- [ ] Leader
  - [ ] Send actual entries in AppendEntries (leader currently ships `vec![]`)
    - ship the full log tail `log[next_idx..]` with matching `prevLogTermIdx`;
    handles multi-entry replication and backfilling lagging followers.
- [ ] Integration tests
  - [ ] election -> timeout/heartbeat -> re-election -> commit an entry
  - [ ] Multi-node test harness: route packets between in-memory nodes by `Packet::to()`
- [x] Include peer id in RPC header
- [x] Include idx in AppendEntryResp
- [x] Handle on_recv in leader
- [x] Handle on_recv in follower
- [x] Handle on_recv in candidate

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

