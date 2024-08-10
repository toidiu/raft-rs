use crate::{
    clock::Clock,
    io::Tx,
    log::Term,
    rpc::{AppendEntries, RequestVote, RespRequestVote, Rpc},
    state::inner::Inner,
};
use s2n_codec::{EncoderBuffer, EncoderValue};
use uuid::Uuid;

mod inner;

pub(crate) use inner::Common;

#[derive(Debug)]
pub struct ServerId(String);

impl ServerId {
    pub fn new() -> Self {
        let id = Uuid::new_v4();
        ServerId(id.to_string())
    }
}

// Trick to convert one enum variant into another with a &mut reference.
//
// This is pretty messy but should be safe.
macro_rules! convert_to {
    ($state:ident, $new:path) => {
        let inner = match $state {
            State::Follower(inner) => std::mem::take(inner),
            State::Leader(inner) => std::mem::take(inner),
            State::Candidate(inner) => std::mem::take(inner),
        };
        *$state = $new(inner);
    };
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
pub enum State {
    Follower(Inner),
    Leader(Inner),
    Candidate(Inner),
}

impl State {
    pub fn new(clock: Clock) -> Self {
        // 1: startup
        State::Follower(Inner::new(clock))
    }

    pub fn common(&self) -> &Common {
        match self {
            State::Follower(inner) => &inner.common,
            State::Leader(inner) => &inner.common,
            State::Candidate(inner) => &inner.common,
        }
    }

    pub fn common_mut(&mut self) -> &mut Common {
        match self {
            State::Follower(inner) => &mut inner.common,
            State::Leader(inner) => &mut inner.common,
            State::Candidate(inner) => &mut inner.common,
        }
    }

    pub fn on_timeout<T: Tx>(&mut self, io: &mut T) {
        match self {
            State::Follower(_inner) => {
                // 2: timeout. start election
                self.on_candidate(io);
            }
            State::Leader(_inner) => {
                self.send_heartbeat(io);
            }
            State::Candidate(_inner) => {
                // 3: timeout. new election
                self.on_candidate(io);
            }
        }
    }

    pub fn recv<T: Tx>(&mut self, tx: &mut T, rpc: Rpc) {
        match self {
            State::Follower(inner) => {
                // # Compliance:
                // - Respond to RPCs from candidates and leaders
                Self::on_recv_follower(inner, tx, rpc);
            }
            State::Leader(_inner) => {}
            State::Candidate(_inner) => {}
        }
    }

    fn on_recv_follower<T: Tx>(inner: &mut Inner, tx: &mut T, rpc: Rpc) {
        println!("state: on_recv_follower");

        match rpc {
            Rpc::RequestVote(RequestVote { term: _ }) => {}
            Rpc::RespRequestVote(RespRequestVote {
                term: _,
                vote_granted: _,
            }) => {}
            Rpc::AppendEntries(AppendEntries { term }) => {
                if inner.common.curr_term == term {
                    inner.common.timer.rearm()
                }
            }
        }
    }

    fn on_candidate<T: Tx>(&mut self, tx: &mut T) {
        println!("state: on_candidate");
        convert_to!(self, State::Candidate);
        // TODO: start new election
        let term = self.common().curr_term.0 + 1;
        let mut slice = vec![0; 100];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_request_vote(term).encode_mut(&mut buf);
        tx.send(buf.as_mut_slice().to_vec().into());
    }

    fn send_heartbeat<T: Tx>(&mut self, tx: &mut T) {
        println!("state: send_heartbeat");

        // TODO send rpc
        // tx.send(Rpc::new_append_entry(1).into());
        let term = self.common().curr_term.0 + 1;
        let mut slice = vec![0; 100];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_append_entry(term).encode_mut(&mut buf);
        tx.send(buf.as_mut_slice().to_vec().into());
    }

    fn on_request_vote(&mut self, rpc: RequestVote) {
        println!("state: recv RequestVote. {:?}", rpc.term);
        // TODO: recv vote, request for new election
        println!("state: recv RequestVote. {:?}", rpc.term);
    }

    fn on_append_entry(&mut self, rpc: AppendEntries) {
        println!("recv AppendEntries. {:?}", rpc.term);
        // TODO: heartbeat, new entry, discover current leader, discover new term
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{io::testing::MockIo, rpc::Rpc, testing::cast};
    use s2n_codec::{DecoderBuffer, DecoderValue};

    #[tokio::test]
    async fn default_state() {
        let s = State::new(Clock::default());
        assert!(matches!(s, State::Follower(_)));
    }

    #[tokio::test]
    async fn follower_timeout() {
        let mut io = MockIo::new();
        let mut s = State::new(Clock::default());
        assert!(matches!(s, State::Follower(_)));

        s.on_timeout(&mut io);
        assert!(matches!(s, State::Candidate(_)));
        let bytes = io.tx.pop_front().unwrap();
        let buf = DecoderBuffer::new(&bytes);
        let (rpc, _buffer) = Rpc::decode(buf).expect("todo");
        let req = cast!(rpc, Rpc::RequestVote);
        assert_eq!(req.term, Term(1));
    }

    #[tokio::test]
    async fn candidate_timeout() {
        let mut io = MockIo::new();
        let mut s = State::new(Clock::default());
        let s_ref = &mut s;
        convert_to!(s_ref, State::Candidate);
        assert!(matches!(s, State::Candidate(_)));

        s.on_timeout(&mut io);
        assert!(matches!(s, State::Candidate(_)));
        let bytes = io.tx.pop_front().unwrap();
        let buf = DecoderBuffer::new(&bytes);
        let (rpc, _buffer) = Rpc::decode(buf).expect("todo");
        let req = cast!(rpc, Rpc::RequestVote);
        assert_eq!(req.term, Term(1));
    }
}
