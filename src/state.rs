use crate::{
    clock::Clock,
    io::ServerTx,
    log::Term,
    rpc::{AppendEntries, RequestVote, RespRequestVote, Rpc},
    state::inner::Inner,
};
use s2n_codec::{EncoderBuffer, EncoderValue};
use uuid::Uuid;

mod inner;

#[derive(Debug)]
pub struct ServerId(String);

impl ServerId {
    pub fn new() -> Self {
        let id = Uuid::new_v4();
        ServerId(id.to_string())
    }
}

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

#[derive(Debug)]
pub struct State {
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
            inner: Inner::new(clock),
            mode: Mode::Follower,
        }
    }

    pub fn on_timeout<T: ServerTx>(&mut self, io: &mut T) {
        match self.mode {
            Mode::Follower => {
                // 2: timeout. start election
                self.on_candidate(io);
            }
            Mode::Leader => {
                self.send_heartbeat(io);
            }
            Mode::Candidate => {
                // 3: timeout. new election
                self.on_candidate(io);
            }
        }
    }

    pub fn recv<T: ServerTx>(&mut self, tx: &mut T, rpc: Rpc) {
        match self.mode {
            Mode::Follower => {
                // # Compliance:
                // - Respond to RPCs from candidates and leaders
                Self::on_follower_recv(&mut self.inner, tx, rpc);
            }
            Mode::Leader => {}
            Mode::Candidate => {}
        }
    }

    fn on_follower_recv<T: ServerTx>(inner: &mut Inner, _tx: &mut T, rpc: Rpc) {
        // println!("state: on_recv_follower");

        match rpc {
            Rpc::RequestVote(RequestVote { term: _ }) => {}
            Rpc::RespRequestVote(RespRequestVote {
                term: _,
                vote_granted: _,
            }) => {}
            Rpc::AppendEntries(AppendEntries { term }) => {
                if inner.curr_term == term {
                    println!("-----------------------i8");
                    inner.timer.rearm()
                }
            }
        }
    }

    fn on_candidate<T: ServerTx>(&mut self, tx: &mut T) {
        self.mode = Mode::Candidate;

        // TODO: start new election
        let term = self.inner.curr_term.0 + 1;
        let mut slice = vec![0; 100];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_request_vote(term).encode_mut(&mut buf);
        tx.send(buf.as_mut_slice().to_vec());
    }

    fn send_heartbeat<T: ServerTx>(&mut self, tx: &mut T) {
        // println!("state: send_heartbeat");

        // TODO send rpc
        let term = self.inner.curr_term.0 + 1;
        let mut slice = vec![0; 100];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_append_entry(term).encode_mut(&mut buf);
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
    use crate::{io::testing::MockIo, rpc::Rpc, testing::cast};
    use core::time::Duration;
    use s2n_codec::{DecoderBuffer, DecoderValue};

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
        let (rpc, _buffer) = Rpc::decode(buf).expect("todo");
        let req = cast!(rpc, Rpc::RequestVote);
        assert_eq!(req.term, Term(1));
    }

    #[tokio::test]
    async fn follower_recv_rearm() {
        let mut io = MockIo::new();
        let mut s = State::new(Clock::default());
        assert!(matches!(s.mode, Mode::Follower));

        let prev_expire = s.inner.timer.expire.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;

        s.recv(&mut io, Rpc::AppendEntries(AppendEntries { term: Term(0) }));
        let new_expire = s.inner.timer.expire.unwrap();
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
        let (rpc, _buffer) = Rpc::decode(buf).expect("todo");
        let req = cast!(rpc, Rpc::RequestVote);
        assert_eq!(req.term, Term(1));
    }
}
