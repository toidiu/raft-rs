use crate::{
    clock::Clock,
    io::{ServerTx, IO_BUF_LEN},
    log::Term,
    rpc::{AppendEntries, Heartbeat, RequestVote, RespAppendEntries, RespRequestVote, Rpc},
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
    Leader,
    Candidate,
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
            Mode::Leader => {
                self.send_heartbeat(io);
            }
            Mode::Candidate => {
                // # Compliance:
                // If election timeout elapses without receiving AppendEntries RPC from current
                // leader or granting vote to candidate: convert to candidate
                self.on_candidate(io);
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
            Mode::Leader => {}
            Mode::Candidate => {}
        }
    }

    fn on_follower_recv<T: ServerTx>(&mut self, tx: &mut T, rpc: Rpc) {
        match rpc {
            Rpc::RequestVote(RequestVote {
                term: _,
                candidate_id,
            }) => {
                let term = self.inner.curr_term.0;
                let mut slice = vec![0; IO_BUF_LEN];
                let mut buf = EncoderBuffer::new(&mut slice);
                #[allow(clippy::match_like_matches_macro)]

                // # Compliance:
                // If votedFor is null or candidateId, grant vote (§5.2, §5.4)
                // TODO on_request_vote
                let voted_for = match &self.inner.voted_for {
                    Some(voted_for) if voted_for == &candidate_id => true,
                    None => {
                        self.inner.voted_for = Some(candidate_id);
                        true
                    }
                    _ => false
                };

                Rpc::new_request_vote_resp(term, voted_for).encode_mut(&mut buf);
                tx.send(buf.as_mut_slice().to_vec());
            }
            Rpc::RespRequestVote(RespRequestVote {
                term: _,
                vote_granted: _,
            }) => {}
            Rpc::AppendEntries(_entery) => {
                let term = self.inner.curr_term.0;
                let mut slice = vec![0; IO_BUF_LEN];
                let mut buf = EncoderBuffer::new(&mut slice);
                Rpc::new_append_entry_resp(term).encode_mut(&mut buf);
                tx.send(buf.as_mut_slice().to_vec());
            }
            Rpc::RespAppendEntries(RespAppendEntries { term: _ }) => {}
            Rpc::Heartbeat(Heartbeat { term: _ }) => {}
        }
    }

    fn on_candidate<T: ServerTx>(&mut self, tx: &mut T) {
        println!("on_candidate");
        self.mode = Mode::Candidate;

        // TODO: start new election
        let term = self.inner.curr_term.0 + 1;
        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_request_vote(term, self.id).encode_mut(&mut buf);
        tx.send(buf.as_mut_slice().to_vec());
    }

    fn on_follower(&mut self) {
        println!("on_follower");
        self.mode = Mode::Follower;
    }

    fn send_heartbeat<T: ServerTx>(&mut self, tx: &mut T) {
        // println!("state: send_heartbeat");

        // TODO send rpc
        let term = self.inner.curr_term.0 + 1;
        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_heartbeat(term).encode_mut(&mut buf);
        tx.send(buf.as_mut_slice().to_vec());
    }

    fn on_request_vote(&mut self, _rpc: RequestVote) {
        // println!("state: recv RequestVote. {:?}", rpc.term);
        // TODO: recv vote, request for new election
    }

    fn on_append_entry(&mut self, _rpc: AppendEntries) {
        // println!("recv AppendEntries. {:?}", rpc.term);
        // TODO: heartbeat, new entry, discover current leader, discover new term
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{io::testing::MockIo, log::TermIdx, rpc::Rpc, testing::cast};
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
        let mut io = MockIo::new();
        let mut s = State::new(Clock::default());
        assert!(matches!(s.mode, Mode::Follower));

        s.on_timeout(&mut io);
        assert!(matches!(s.mode, Mode::Candidate));
        let bytes = io.tx.pop_front().unwrap();
        let buf = DecoderBuffer::new(&bytes);
        let (rpc, _buffer) = Rpc::decode(buf).unwrap();
        let req = cast!(rpc, Rpc::RequestVote);
        assert_eq!(req.term, Term(1));
    }

    #[tokio::test]
    async fn follower_recv_rearm() {
        let mut io = MockIo::new();
        let mut s = State::new(Clock::default());
        assert!(matches!(s.mode, Mode::Follower));

        let prev_expire = s.inner.timer.expire();

        advance(Duration::from_millis(500)).await;
        s.recv(&mut io, Rpc::new_append_entry(0, TermIdx::new(0, 1)));
        let new_expire = s.inner.timer.expire();
        assert!(new_expire > prev_expire);
    }

    #[tokio::test]
    async fn candidate_timeout() {
        let mut io = MockIo::new();
        let mut s = State::new(Clock::default());
        s.mode = Mode::Candidate;

        s.on_timeout(&mut io);
        assert!(matches!(s.mode, Mode::Candidate));
        let bytes = io.tx.pop_front().unwrap();
        let buf = DecoderBuffer::new(&bytes);
        let (rpc, _buffer) = Rpc::decode(buf).unwrap();
        let req = cast!(rpc, Rpc::RequestVote);
        assert_eq!(req.term, Term(1));
    }
}
