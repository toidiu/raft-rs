# raft-rs

A toy implementation to better understand the
[Raft](https://toidiu.com/reads/In_Search_of_an_Understandable_Consensus_Algorithm_(Extended_Raft).pdf)
consensus protocol

TODO:
- [x] Include peer id in RPC header
- [x] Include idx in AppendEntryResp
- [ ] Add leader test
    - [ ] on_timeout
    - [ ] leader progression
- [ ] Handle on_recv in leader
- [ ] Handle on_recv in follower
- [ ] Handle on_recv in candidate
- [ ] Tests for Leader on_recv_append_entry_resp

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

