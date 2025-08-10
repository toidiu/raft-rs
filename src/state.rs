use crate::{
    clock::Clock,
    io::ServerTx,
    log::Term,
    rpc::{AppendEntries, RequestVote, RespAppendEntries, RespRequestVote, Rpc},
    state::inner::{ElectionResult, Inner},
};
use s2n_codec::{DecoderValue, EncoderValue};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServerId(pub [u8; 16]);

impl ServerId {
    pub fn new() -> Self {
        let id = Uuid::new_v4();
        ServerId(id.into_bytes())
    }
}

impl<'a> DecoderValue<'a> for ServerId {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> s2n_codec::DecoderBufferResult<'a, Self> {
        let (candidate_id, buffer) = buffer.decode_slice(16)?;
        let candidate_id = candidate_id
            .into_less_safe_slice()
            .try_into()
            .expect("already decoded 16 bytes");
        Ok((ServerId(candidate_id), buffer))
    }
}

impl EncoderValue for ServerId {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.write_slice(&self.0)
    }
}

#[derive(Debug)]
pub struct State {
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
    pub fn new(clock: Clock, server_list: Vec<ServerId>) -> Self {
        // 1: startup
        State {
            inner: Inner::new(clock, server_list),
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
            Mode::Leader => {}
        }
    }

    pub fn recv<T: ServerTx>(&mut self, tx: &mut T, rpc: Rpc) {
        // TODO validate the term and idx. or is this done in on_request_vote/on_append_entry?

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
                id: _,
                vote_granted: _,
            }) => {
                todo!()
            }
            Rpc::AppendEntries(append_entry) => {
                self.on_append_entry(append_entry, tx);
            }
            Rpc::RespAppendEntries(RespAppendEntries { term: _, .. }) => {
                todo!()
            }
        }
    }

    fn on_candidate_recv<T: ServerTx>(&mut self, tx: &mut T, rpc: Rpc) {
        match rpc {
            Rpc::RequestVote(request_vote) => self.on_request_vote(request_vote, tx),
            Rpc::RespRequestVote(RespRequestVote {
                term: _,
                id,
                vote_granted,
            }) => {
                if vote_granted {
                    let result = self.inner.on_vote_received(id);
                    if matches!(result, ElectionResult::Elected) {
                        self.on_leader(tx);
                    }
                }
            }
            Rpc::AppendEntries(append_entry) => {
                let rpc_term = append_entry.term();
                // # Compliance:
                // If AppendEntries RPC received from new leader: convert to follower
                let new_leader_detected = rpc_term > self.inner.curr_term;
                if new_leader_detected {
                    self.on_follower();
                    self.on_append_entry(append_entry, tx);
                }
            }
            Rpc::RespAppendEntries(RespAppendEntries { term: _, .. }) => {
                todo!()
            }
        }
    }

    fn on_candidate<T: ServerTx>(&mut self, tx: &mut T) {
        println!("on_candidate");
        self.mode = Mode::Candidate;

        // # Compliance:
        // On conversion to candidate, start election:
        self.on_new_election(tx)
    }

    fn on_leader<T: ServerTx>(&mut self, _tx: &mut T) {
        println!("on_leader");
        self.mode = Mode::Leader;
    }

    fn on_new_election<T: ServerTx>(&mut self, tx: &mut T) {
        self.inner.on_new_election();

        // # Compliance:
        // Increment currentTerm
        self.inner.curr_term += 1;

        // # Compliance:
        // Vote for self
        if matches!(self.inner.cast_vote(self.inner.id), ElectionResult::Elected) {
            self.on_leader(tx);
        }

        // # Compliance:
        // Reset election timer
        self.inner.timer.rearm();

        // # Compliance:
        // Send RequestVote RPCs to all other servers
        let term = self.inner.curr_term.0;
        let last_committed_term_idx = self.inner.last_committed_term_idx();
        let rpc = Rpc::new_request_vote(term, self.inner.id, last_committed_term_idx);
        tx.send_rpc(rpc);
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
        debug_assert!(
            candidate_id != self.inner.id,
            "sanity check that the peer's id is different from self"
        );

        // # Compliance:
        // Reply false if term < currentTerm (§5.1)
        let term_up_to_date = rpc_term >= self.inner.curr_term;

        // # Compliance:
        // If candidate’s log is at least as up-to-date as receiver’s log
        let logs_up_to_date = rpc_last_log_term_idx >= self.inner.last_committed_term_idx();

        let give_vote = match &self.inner.voted_for() {
            Some(voted_for) if voted_for == &candidate_id => {
                // # Compliance:
                // and votedFor is candidateId, grant vote (§5.2, §5.4)
                true
            }
            None => {
                // # Compliance:
                // and votedFor is null, grant vote (§5.2, §5.4)
                let voted_for_peer = self.inner.cast_vote(candidate_id);
                debug_assert!(
                    matches!(voted_for_peer, ElectionResult::Pending),
                    "voted for peer so election should be pending"
                );
                true
            }
            _ => false,
        };

        let voted_for_resp = term_up_to_date && logs_up_to_date && give_vote;

        let term = self.inner.curr_term.0;
        let rpc = Rpc::new_request_vote_resp(term, ServerId::new(), voted_for_resp);
        tx.send_rpc(rpc);
    }

    fn on_append_entry<T: ServerTx>(&mut self, rpc: AppendEntries, tx: &mut T) {
        let AppendEntries {
            term: rpc_term,
            prev_term_idx: rcp_prev_term_idx,
        } = rpc;

        // # Compliance: Fig 2
        // Reply false if term < currentTerm (§5.1)
        let term_up_to_date = rpc_term >= self.inner.curr_term;

        // # Compliance: Fig 2
        // prevLogIndex index of log entry immediately preceding new ones
        //
        // Since we haven't processed and of the new entries yet, this is the last entry in the log
        let prev_term_idx = self.inner.last_committed_term_idx();

        // # Compliance: Fig 2
        // Reply false if log doesn’t contain an entry at prevLogIndex whose term matches
        // prevLogTerm (§5.3)
        //
        // # Compliance: Fig 2
        // success true if follower contained entry matching prevLogIndex and prevLogTerm
        let log_contains_entry = rcp_prev_term_idx == prev_term_idx;

        let success = term_up_to_date && log_contains_entry;

        let term = self.inner.curr_term.0;
        let rpc = Rpc::new_append_entry_resp(term, success);
        tx.send_rpc(rpc);
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
        let s = State::new(Clock::default(), vec![]);
        assert!(matches!(s.mode, Mode::Follower));
    }

    #[tokio::test]
    async fn follower_timeout() {
        let mut io = testing::Io::new();
        let mut s = State::new(Clock::default(), vec![ServerId::new()]);
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
        let mut s = State::new(Clock::default(), vec![]);

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
        let mut s = State::new(Clock::default(), vec![]);

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
        let mut s = State::new(Clock::default(), vec![]);
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
        let mut s = State::new(Clock::default(), vec![ServerId::new()]);
        s.mode = Mode::Candidate;

        s.on_timeout(&mut io);
        assert!(matches!(s.mode, Mode::Candidate));
        let bytes = io.tx.pop_front().unwrap();
        let buf = DecoderBuffer::new(&bytes);
        let (rpc, _buffer) = Rpc::decode(buf).unwrap();
        let req = cast_unsafe!(rpc, Rpc::RequestVote);
        assert_eq!(req.term, Term(1));
    }

    #[tokio::test]
    async fn majority_of_votes() {
        let mut io = testing::Io::new();
        // 2 peers in addition to the current server
        let server_list = vec![ServerId::new(), ServerId::new()];
        let mut s = State::new(Clock::default(), server_list.clone());

        assert!(s.inner.voted_for().is_none());

        // on_timeout start a new election
        s.on_timeout(&mut io);
        assert!(matches!(s.mode, Mode::Candidate));
        assert_eq!(s.inner.votes_received.len(), 1);
        // not Elected
        assert!(matches!(s.mode, Mode::Candidate));

        // RequestVote is sent when a new election is started
        let bytes = io.tx.pop_front().unwrap();
        let buf = DecoderBuffer::new(&bytes);
        let (rpc, _buffer) = Rpc::decode(buf).unwrap();
        let req = cast_unsafe!(rpc, Rpc::RequestVote);
        assert_eq!(req.term, Term(1));
        assert!(io.tx.is_empty());

        // recv RespRequestVote, second vote give us quorum
        s.recv(&mut io, Rpc::new_request_vote_resp(1, server_list[0], true));
        assert_eq!(s.inner.votes_received.len(), 2);
        // Elected
        assert!(matches!(s.mode, Mode::Leader));
    }

    #[tokio::test]
    async fn single_server_wins_election() {
        let mut io = testing::Io::new();
        let mut s = State::new(Clock::default(), vec![]);
        assert!(matches!(s.mode, Mode::Follower));

        s.on_timeout(&mut io);
        // Elected since there are no peers
        assert!(matches!(s.mode, Mode::Leader));
    }
}
