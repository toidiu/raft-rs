use crate::{
    io::ServerEgress,
    log::{Term, TermIdx},
    packet::Rpc,
    raft_state::RaftState,
    server::{Id, PeerId},
};
use s2n_codec::{DecoderValue, EncoderValue};

#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub struct RequestVote {
    //% Compliance:
    //% term: candidate’s term
    pub term: Term,

    //% Compliance:
    //% candidateId: candidate requesting vote
    pub candidate_id: Id,

    //% Compliance:
    //% lastLogIndex: index of candidate’s last log entry (§5.4)
    //% lastLogTerm: term of candidate’s last log entry (§5.4
    pub last_log_term_idx: TermIdx,
}

impl RequestVote {
    pub const TAG: u8 = 1;

    pub fn on_recv<E: ServerEgress>(
        &self,
        peer_id: PeerId,
        raft_state: &mut RaftState,
        io_egress: &mut E,
    ) {
        let current_term = raft_state.current_term;

        //% Compliance:
        //% Reply false if term < currentTerm (§5.1)
        let term_criteria = {
            let rpc_term_lt_current_term = self.term < current_term;
            !rpc_term_lt_current_term
        };

        //% Compliance:
        //% If candidate’s log is at least as up-to-date as receiver’s log
        //
        //% Compliance:
        //% The RequestVote RPC helps ensure the leader's log is `up-to-date`
        //% -	RequestVote includes info about candidate's log
        //% -	voter denies vote if its own log is more `up-to-date`
        let log_up_to_date_criteria = raft_state
            .log
            .is_candidate_log_up_to_date(&self.last_log_term_idx);

        let voted_for_criteria = if let Some(voted_for) = raft_state.voted_for() {
            //% Compliance:
            //% and votedFor is candidateId, grant vote (§5.2, §5.4)
            voted_for.eq(&self.candidate_id)
        } else {
            //% Compliance:
            //% and votedFor is null, grant vote (§5.2, §5.4)
            true
        };

        let grant_vote = term_criteria && log_up_to_date_criteria && voted_for_criteria;
        if grant_vote {
            // set local state to capture granting the vote
            //
            // SAFETY: Receiving the RequestVote RPC means voting for a peer.
            let vote_for_candidate = unsafe { self.candidate_id.as_peer_id() };
            raft_state.voted_for_peer(vote_for_candidate);
        }

        let rpc = Rpc::new_request_vote_resp(current_term, grant_vote);
        io_egress.send_packet(peer_id, rpc);
    }
}

#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub struct RequestVoteResp {
    //% Compliance:
    //% term: currentTerm, for candidate to update itself
    pub term: Term,

    //% Compliance:
    //% voteGranted: true means candidate received vote
    pub vote_granted: bool,
}

impl RequestVoteResp {
    pub const TAG: u8 = 2;
}

impl<'a> DecoderValue<'a> for RequestVote {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> s2n_codec::DecoderBufferResult<'a, Self> {
        let (term, buffer) = buffer.decode()?;
        let (candidate_id, buffer) = buffer.decode()?;
        let (last_log_term_idx, buffer) = buffer.decode()?;

        let rpc = RequestVote {
            term,
            candidate_id,
            last_log_term_idx,
        };
        Ok((rpc, buffer))
    }
}

impl EncoderValue for RequestVote {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.term);
        encoder.encode(&self.candidate_id);
        encoder.encode(&self.last_log_term_idx);
    }
}

impl<'a> DecoderValue<'a> for RequestVoteResp {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> s2n_codec::DecoderBufferResult<'a, Self> {
        let (term, buffer) = buffer.decode()?;
        let (vote_granted, buffer): (u8, _) = buffer.decode()?;
        let vote_granted = vote_granted != 0;

        let rpc = RequestVoteResp { term, vote_granted };
        Ok((rpc, buffer))
    }
}

impl EncoderValue for RequestVoteResp {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.term);
        encoder.write_slice(&(self.vote_granted as u8).to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        io::testing::{helper_inspect_one_sent_packet, MockIo},
        log::{Entry, Idx, Term, TermIdx},
        macros::cast_unsafe,
        raft_state::RaftState,
        server::{PeerId, ServerId},
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;
    use s2n_codec::{DecoderBuffer, EncoderBuffer};

    #[test]
    fn encode_decode_rpc() {
        let rpc = RequestVote {
            term: Term::from(2),
            candidate_id: Id::new([21; 16]),
            last_log_term_idx: TermIdx::builder()
                .with_term(Term::from(3))
                .with_idx(Idx::from(4)),
        };

        let mut slice = vec![0; 40];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc, _) = RequestVote::decode(d_buf).unwrap();

        assert_eq!(rpc, d_rpc);
    }

    #[test]
    fn encode_decode_rpc_resp() {
        let rpc = RequestVoteResp {
            term: Term::from(2),
            vote_granted: true,
        };

        let mut slice = vec![0; 30];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc, _) = RequestVoteResp::decode(d_buf).unwrap();

        assert_eq!(rpc, d_rpc);
    }

    #[tokio::test]
    async fn test_recv_request_vote_up_to_date_criteria_with_empty_log() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let candidate_id = ServerId::new([2; 16]);
        let peer_id = PeerId::new([11; 16]);
        let mut state = RaftState::new(timeout);
        let current_term = Term::initial();
        state.current_term = current_term;
        assert!(state.log.is_empty());

        let mut io = MockIo::new(candidate_id);
        let rpc_term_idx_initial = TermIdx::initial();

        // Expect grant_vote: empty Log, TermIdx initial
        {
            // construct RPC to recv
            let recv_rpc = Rpc::new_request_vote(current_term, candidate_id, rpc_term_idx_initial);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(current_term, true);
            assert_eq!(&expected_rpc, packet.rpc());
        }

        // Expect grant_vote: empty Log, TermIdx > initial
        {
            // construct RPC to recv
            let rpc_last_log_term_idx = TermIdx::builder()
                .with_term(Term::from(1))
                .with_idx(Idx::from(1));
            let rpc_term = Term::from(1);
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_last_log_term_idx);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(current_term, true);
            assert_eq!(&expected_rpc, packet.rpc());
        }
    }

    #[tokio::test]
    async fn test_recv_request_vote_up_to_date_criteria() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let candidate_id = ServerId::new([2; 16]);
        let peer_id = PeerId::new([11; 16]);
        let mut state = RaftState::new(timeout);

        let idx_lt = Idx::from(1);
        let idx_eq = Idx::from(2);
        let idx_gt = Idx::from(3);
        let term_prev = Term::from(1);
        let term_current = Term::from(2);
        let term_ahead = Term::from(3);
        state.current_term = term_current;

        // local Log: [ [term: 1] [term: 2] ]
        state
            .log
            .push(vec![Entry::new(term_prev, 3), Entry::new(term_current, 6)]);

        let mut io = MockIo::new(candidate_id);

        // == Equal TermIdx ==
        // Expect: grant vote
        {
            let rpc_term = term_current;
            let rpc_idx = idx_eq;

            // construct RPC to recv
            let rpc_last_log_term_idx = TermIdx::builder().with_term(rpc_term).with_idx(rpc_idx);
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_last_log_term_idx);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(term_current, true);
            assert_eq!(&expected_rpc, packet.rpc());
        }

        // == Different Term ==
        // term <
        // Expect: NO grant vote
        {
            let rpc_term = term_prev;
            let rpc_idx = idx_eq;

            // construct RPC to recv
            let rpc_last_log_term_idx = TermIdx::builder().with_term(rpc_term).with_idx(rpc_idx);
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_last_log_term_idx);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(term_current, false);
            assert_eq!(&expected_rpc, packet.rpc());
        }

        // == Different Term ==
        // term >
        // Expect: grant vote
        {
            let rpc_term = term_ahead;
            let rpc_idx = idx_eq;

            // construct RPC to recv
            let rpc_last_log_term_idx = TermIdx::builder().with_term(rpc_term).with_idx(rpc_idx);
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_last_log_term_idx);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(term_current, true);
            assert_eq!(&expected_rpc, packet.rpc());
        }

        // == Same Term ==
        // idx <
        // Expect: NO grant vote
        {
            let rpc_term = term_current;
            let rpc_idx = idx_lt;

            // construct RPC to recv
            let rpc_last_log_term_idx = TermIdx::builder().with_term(rpc_term).with_idx(rpc_idx);
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_last_log_term_idx);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(term_current, false);
            assert_eq!(&expected_rpc, packet.rpc());
        }

        // == Same Term ==
        // idx >
        // Expect: grant vote
        {
            let rpc_term = term_current;
            let rpc_idx = idx_gt;

            // construct RPC to recv
            let rpc_last_log_term_idx = TermIdx::builder().with_term(rpc_term).with_idx(rpc_idx);
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_last_log_term_idx);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(term_current, true);
            assert_eq!(&expected_rpc, packet.rpc());
        }
    }

    #[tokio::test]
    async fn test_recv_request_vote_vote_granted_criteria() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let candidate_id = ServerId::new([2; 16]);
        let peer_id = PeerId::new([11; 16]);
        let mut state = RaftState::new(timeout);

        let term_current = Term::from(2);
        state.current_term = term_current;

        let mut io = MockIo::new(candidate_id);

        let rpc_term = term_current;
        let rpc_last_log_term_idx = TermIdx::initial();
        // == vote_granted None ==
        // Expect: grant vote
        {
            assert!(state.voted_for().is_none());

            // construct RPC to recv
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_last_log_term_idx);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(term_current, true);
            assert_eq!(&expected_rpc, packet.rpc());
        }

        // == vote_granted Some(candidate_id) ==
        // Expect: grant vote
        {
            state.voted_for_self(candidate_id);

            // construct RPC to recv
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_last_log_term_idx);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(term_current, true);
            assert_eq!(&expected_rpc, packet.rpc());
        }

        // == vote_granted Some(!candidate_id) ==
        // Expect: NO grant vote
        {
            state.voted_for_peer(peer_id);

            // construct RPC to recv
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_last_log_term_idx);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(term_current, false);
            assert_eq!(&expected_rpc, packet.rpc());
        }
    }

    #[tokio::test]
    async fn test_recv_request_vote_term_criteria() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());

        let candidate_id = ServerId::new([2; 16]);
        let peer_id = PeerId::new([11; 16]);
        let mut state = RaftState::new(timeout);

        let term_prev = Term::from(1);
        let term_current = Term::from(2);
        let term_ahead = Term::from(3);
        state.current_term = term_current;

        let mut io = MockIo::new(candidate_id);

        let rpc_last_log_term_idx = TermIdx::initial();

        // == rpc_term = current_term ==
        // Expect: grant vote
        {
            let rpc_term = term_current;

            // construct RPC to recv
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_last_log_term_idx);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(term_current, true);
            assert_eq!(&expected_rpc, packet.rpc());
        }

        // == rpc_term > current_term ==
        // Expect: grant vote
        {
            let rpc_term = term_ahead;

            // construct RPC to recv
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_last_log_term_idx);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(term_current, true);
            assert_eq!(&expected_rpc, packet.rpc());
        }

        // == rpc_term < current_term ==
        // Expect: NO grant vote
        {
            let rpc_term = term_prev;

            // construct RPC to recv
            let recv_rpc = Rpc::new_request_vote(rpc_term, candidate_id, rpc_last_log_term_idx);
            cast_unsafe!(recv_rpc, Rpc::RequestVote).on_recv(peer_id, &mut state, &mut io);

            let packet = helper_inspect_one_sent_packet(&mut io);
            let expected_rpc = Rpc::new_request_vote_resp(term_current, false);
            assert_eq!(&expected_rpc, packet.rpc());
        }
    }
}
