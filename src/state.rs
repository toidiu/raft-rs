use crate::{
    clock::Clock,
    io::{ServerTx, IO_BUF_LEN},
    log::Term,
    rpc::{AppendEntries, RequestVote, RespAppendEntries, RespRequestVote, Rpc},
    state::inner::Inner,
};
use s2n_codec::{EncoderBuffer, EncoderValue};
use uuid::Uuid;

mod inner;

/// Raft state diagram.
///
/// 1: startup
/// 2: timeout. start election
/// 3: timeout. new election
/// 4: recv vote from majority of servers
/// 5: discover current leader or new term
/// 6: discover server with higher term
///
///
/// ```none
///
///     |                       ------
///     | 1                    |  3   |
///     v             2        |      v
/// +----------+ --------->  +-----------+
/// |          |             |           |
/// | Follower |             | Candidate |
/// |          |             |           |
/// +----------+  <--------- +-----------+
///        ^          5             |
///        |                        | 4
///        |                        v
///        |          6        +--------+
///         ------------------ |        |
///                            | Leader |
///                            |        |
///                            +--------+
///
/// ```
/// https://textik.com/#8dbf6540e0dd1676

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerId(pub [u8; 16]);

impl ServerId {
    pub fn new() -> Self {
        let id = Uuid::new_v4();
        ServerId(id.into_bytes())
    }
}

#[derive(Debug)]
pub struct State {
    pub id: ServerId,
    pub inner: Inner,
    mode: Mode,
}

#[derive(Debug)]
enum Mode {
    Follower,
    Candidate,
    Leader,
}

impl State {
    pub fn new(clock: Clock) -> Self {
        // 1: startup
        State {
            id: ServerId::new(),
            inner: Inner::new(clock),
            mode: Mode::Follower,
        }
    }

    pub fn on_timeout<T: ServerTx>(&mut self, io: &mut T) {
        match self.mode {
            Mode::Follower => {
                // # Compliance:
                // If election timeout elapses without receiving AppendEntries RPC from current
                // leader or granting vote to candidate: convert to candidate
                self.on_candidate(io);
            }
            Mode::Candidate => {
                // # Compliance:
                // If election timeout elapses: start new election
                self.on_candidate(io);
            }
            Mode::Leader => {
            }
        }
    }

    pub fn recv<T: ServerTx>(&mut self, tx: &mut T, rpc: Rpc) {
        if rpc.term() >= &self.inner.curr_term {
            self.inner.timer.rearm();
        }

        // # Compliance:
        // If RPC request or response contains term T > currentTerm: set currentTerm = T, convert
        // to follower (§5.1)
        if rpc.term() > &self.inner.curr_term {
            self.inner.curr_term = *rpc.term();
            self.on_follower();
        }

        match self.mode {
            Mode::Follower => {
                // # Compliance:
                // - Respond to RPCs from candidates and leaders
                Self::on_follower_recv(self, tx, rpc);
            }
            Mode::Candidate => {
                Self::on_candidate_recv(self, tx, rpc);
            }
            Mode::Leader => {}
        }
    }

    fn on_follower_recv<T: ServerTx>(&mut self, tx: &mut T, rpc: Rpc) {
        match rpc {
            Rpc::RequestVote(request_vote) => {
                self.on_request_vote(request_vote, tx);
            }
            Rpc::RespRequestVote(RespRequestVote {
                term: _,
                vote_granted: _,
            }) => {}
            Rpc::AppendEntries(append_entry) => {
                self.on_append_entry(append_entry, tx);
            }
            Rpc::RespAppendEntries(RespAppendEntries { term: _ }) => {}
        }
    }

    fn on_candidate_recv<T: ServerTx>(&mut self, tx: &mut T, rpc: Rpc) {
        match rpc {
            Rpc::RequestVote(request_vote) => {
                self.on_request_vote(request_vote, tx)
            }
            Rpc::RespRequestVote(RespRequestVote {
                term: _,
                vote_granted: _,
            }) => {}
            Rpc::AppendEntries(append_entry) => {
                let rpc_term = append_entry.term();
                // # Compliance:
                // If AppendEntries RPC received from new leader: convert to follower
                let new_leader_detected = rpc_term >= self.inner.curr_term;
                if new_leader_detected {
                    self.on_follower();
                }
            }
            Rpc::RespAppendEntries(RespAppendEntries { term: _ }) => {}
        }
    }

    fn on_candidate<T: ServerTx>(&mut self, tx: &mut T) {
        println!("on_candidate");
        self.mode = Mode::Candidate;

        // # Compliance:
        // On conversion to candidate, start election:
        self.start_new_election(tx)
    }

    fn start_new_election<T: ServerTx>(&mut self, tx: &mut T) {
        // # Compliance:
        // Increment currentTerm
        self.inner.curr_term += 1;

        // # Compliance:
        // Vote for self
        self.inner.voted_for = Some(self.id);

        // # Compliance:
        // Reset election timer
        self.inner.timer.rearm();

        // # Compliance:
        // Send RequestVote RPCs to all other servers
        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        let term = self.inner.curr_term.0;
        let last_committed_term_idx = self.inner.last_committed_term_idx();
        Rpc::new_request_vote(term, self.id, last_committed_term_idx).encode_mut(&mut buf);
        tx.send(buf.as_mut_slice().to_vec());
    }

    fn on_follower(&mut self) {
        println!("on_follower");
        self.mode = Mode::Follower;
    }

    fn on_request_vote<T: ServerTx>(&mut self, request_vote: RequestVote, tx: &mut T) {
        let RequestVote {
            term: rpc_term,
            candidate_id,
            last_log_term_idx: rpc_last_log_term_idx,
        } = request_vote;
        // # Compliance:
        // Reply false if term < currentTerm (§5.1)
        let term_up_to_date = rpc_term >= self.inner.curr_term;

        // # Compliance:
        // If candidate’s log is at least as up-to-date as receiver’s log
        let logs_up_to_date = rpc_last_log_term_idx >= self.inner.last_committed_term_idx();

        let give_vote = match &self.inner.voted_for {
            Some(voted_for) if voted_for == &candidate_id => {
                // # Compliance:
                // and votedFor is candidateId , grant vote (§5.2, §5.4)
                true
            }
            None => {
                self.inner.voted_for = Some(candidate_id);
                // # Compliance:
                // and votedFor is null , grant vote (§5.2, §5.4)
                true
            }
            _ => false,
        };

        let voted_for = term_up_to_date && logs_up_to_date && give_vote;

        let term = self.inner.curr_term.0;
        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_request_vote_resp(term, voted_for).encode_mut(&mut buf);
        tx.send(buf.as_mut_slice().to_vec());
    }

    fn on_append_entry<T: ServerTx>(&mut self, _rpc: AppendEntries, tx: &mut T) {
        let term = self.inner.curr_term.0;
        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_append_entry_resp(term).encode_mut(&mut buf);
        tx.send(buf.as_mut_slice().to_vec());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{io::testing, log::TermIdx, rpc::Rpc, testing::cast_unsafe};
    use core::time::Duration;
    use s2n_codec::{DecoderBuffer, DecoderValue};
    use tokio::time::advance;

    #[tokio::test]
    async fn default_state() {
        let s = State::new(Clock::default());
        assert!(matches!(s.mode, Mode::Follower));
    }

    #[tokio::test]
    async fn follower_timeout() {
        let mut io = testing::Io::new();
        let mut s = State::new(Clock::default());
        assert!(matches!(s.mode, Mode::Follower));

        s.on_timeout(&mut io);
        assert!(matches!(s.mode, Mode::Candidate));
        let bytes = io.tx.pop_front().unwrap();
        let buf = DecoderBuffer::new(&bytes);
        let (rpc, _buffer) = Rpc::decode(buf).unwrap();
        let req = cast_unsafe!(rpc, Rpc::RequestVote);
        assert_eq!(req.term, Term(1));
    }

    #[tokio::test]
    async fn recv_rearm() {
        tokio::time::pause();
        let mut io = testing::Io::new();
        let mut s = State::new(Clock::default());

        let modes = [Mode::Follower, Mode::Candidate, Mode::Leader];
        for mode in modes {
            s.mode = mode;
            let prev_expire = s.inner.timer.expire();

            advance(Duration::from_millis(500)).await;
            s.recv(&mut io, Rpc::new_append_entry(0, TermIdx::new(0, 1)));
            let new_expire = s.inner.timer.expire();
            assert!(new_expire > prev_expire);
        }
    }

    #[tokio::test]
    async fn recv_higher_term() {
        let mut io = testing::Io::new();
        let mut s = State::new(Clock::default());

        // same term maintains current mode
        s.mode = Mode::Candidate;
        s.recv(&mut io, Rpc::new_append_entry(0, TermIdx::new(0, 1)));
        assert!(matches!(s.mode, Mode::Candidate));

        // higher term switch to follower
        s.recv(&mut io, Rpc::new_append_entry(1, TermIdx::new(0, 1)));
        assert!(matches!(s.mode, Mode::Follower));

        // same term maintains current mode
        s.mode = Mode::Leader;
        s.recv(&mut io, Rpc::new_append_entry(1, TermIdx::new(0, 1)));
        assert!(matches!(s.mode, Mode::Leader));

        // higher term switch to follower
        s.recv(&mut io, Rpc::new_append_entry(2, TermIdx::new(0, 1)));
        assert!(matches!(s.mode, Mode::Follower));
    }

    #[tokio::test]
    async fn on_follower_recv() {
        let mut io = testing::Io::new();
        let mut s = State::new(Clock::default());
        assert!(matches!(s.mode, Mode::Follower));

        // recv AppendEntries as a follower
        s.recv(&mut io, Rpc::new_append_entry(0, TermIdx::new(0, 1)));

        // expect follower to respond with RespAppendEntries
        let bytes = io.tx.pop_front().unwrap();
        let buf = DecoderBuffer::new(&bytes);
        let (recv_rpc, _buf) = Rpc::decode(buf).unwrap();
        cast_unsafe!(recv_rpc, Rpc::RespAppendEntries);
    }

    #[tokio::test]
    async fn candidate_timeout() {
        let mut io = testing::Io::new();
        let mut s = State::new(Clock::default());
        s.mode = Mode::Candidate;

        s.on_timeout(&mut io);
        assert!(matches!(s.mode, Mode::Candidate));
        let bytes = io.tx.pop_front().unwrap();
        let buf = DecoderBuffer::new(&bytes);
        let (rpc, _buffer) = Rpc::decode(buf).unwrap();
        let req = cast_unsafe!(rpc, Rpc::RequestVote);
        assert_eq!(req.term, Term(1));
    }
}
