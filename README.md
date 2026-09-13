# raft-rs

A toy implementation to better understand the
[Raft](https://toidiu.com/reads/In_Search_of_an_Understandable_Consensus_Algorithm_(Extended_Raft).pdf)
consensus protocol

TODO:
- [ ] Fuzz test the protocol.
  - [ ] make timeout independent of tokio
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

## Fuzzing
Run the fuzzer with `make fuzz`. Crashes are written to `__fuzz__/<target>/crashes/` and are
committed so they replay on every `cargo test`.

Corpus dirs (`__fuzz__/<target>/corpus/`) are gitignored since they hold hundreds of small files
that change on every run. Commit a tarball of the corpus instead. Run from the repo root so the
tarball stores repo relative paths:

```sh
tar czf sim/src/fuzz/corpus.tar.gz sim/src/fuzz/__fuzz__/fuzz__raft/corpus

# restore
tar xzf sim/src/fuzz/corpus.tar.gz
```

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

