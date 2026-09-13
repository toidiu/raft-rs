# raft-rs

A toy implementation to better understand the
[Raft](https://toidiu.com/reads/In_Search_of_an_Understandable_Consensus_Algorithm_(Extended_Raft).pdf)
consensus protocol

TODO:
- [ ] Fuzz test the protocol.
- [x] sim
  - [x] unique server id
  - [x] faster router
  - [x] large size cluster test
- [ ] add len to packet header
- [ ] queues
  - [ ] ring buffer for queues
  - [ ] queue bounds check rather than overflow
  - [ ] fragmented packets (TCP can deliver a payload over multiple packets)
- [ ] network send should return packet

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

