# raft-rs

A toy implementation to better understand the
[Raft](https://toidiu.com/reads/In_Search_of_an_Understandable_Consensus_Algorithm_(Extended_Raft).pdf)
consensus protocol

TODO:
- [ ] Client API: on the server to propose a command (stores `u8` data)
- [ ] Integration tests
  - [ ] Election -> timeout/heartbeat -> re-election -> commit an entry
  - [ ] Multi-node test harness: route packets between in-memory nodes by `Packet::to()`
- [ ] same timeout of 200ms in cfg(test)
- [ ] packets are not routed between servers yet
- [ ] test: leader progression
- [ ] test: state machine tests
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

