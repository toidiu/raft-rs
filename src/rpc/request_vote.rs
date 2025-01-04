use crate::{
    log::{Term, TermIdx},
    server::ServerId,
};
use s2n_codec::{DecoderValue, EncoderValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestVoteState {
    //% Compliance:
    //% term: candidate’s term
    pub term: Term,

    //% Compliance:
    //% candidateId: candidate requesting vote
    pub candidate_id: ServerId,

    //% Compliance:
    //% lastLogIndex: index of candidate’s last log entry (§5.4)
    //% lastLogTerm: term of candidate’s last log entry (§5.4
    pub last_log_term_idx: TermIdx,
}

impl RequestVoteState {
    pub const TAG: u8 = 1;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespRequestVoteState {
    //% Compliance:
    //% term: currentTerm, for candidate to update itself
    pub term: Term,

    //% Compliance:
    //% voteGranted: true means candidate received vote
    pub vote_granted: bool,
}

impl RespRequestVoteState {
    pub const TAG: u8 = 2;
}

impl<'a> DecoderValue<'a> for RequestVoteState {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> s2n_codec::DecoderBufferResult<'a, Self> {
        let (term, buffer) = buffer.decode()?;
        let (candidate_id, buffer) = buffer.decode()?;
        let (last_log_term_idx, buffer) = buffer.decode()?;

        let rpc = RequestVoteState {
            term,
            candidate_id,
            last_log_term_idx,
        };
        Ok((rpc, buffer))
    }
}

impl EncoderValue for RequestVoteState {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.term);
        encoder.encode(&self.candidate_id);
        encoder.encode(&self.last_log_term_idx);
    }
}

impl<'a> DecoderValue<'a> for RespRequestVoteState {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> s2n_codec::DecoderBufferResult<'a, Self> {
        let (term, buffer) = buffer.decode()?;
        let (vote_granted, buffer): (u8, _) = buffer.decode()?;
        let vote_granted = vote_granted != 0;

        let rpc = RespRequestVoteState { term, vote_granted };
        Ok((rpc, buffer))
    }
}

impl EncoderValue for RespRequestVoteState {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.term);
        encoder.write_slice(&(self.vote_granted as u8).to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::Idx;
    use s2n_codec::{DecoderBuffer, EncoderBuffer};

    #[test]
    fn encode_decode_rpc() {
        let rpc = RequestVoteState {
            term: Term::from(2),
            candidate_id: ServerId::new([10; 16]),
            last_log_term_idx: TermIdx::builder()
                .with_term(Term::from(3))
                .with_idx(Idx::from(4)),
        };

        let mut slice = vec![0; 40];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc, _) = RequestVoteState::decode(d_buf).unwrap();

        assert_eq!(rpc, d_rpc);
    }

    #[test]
    fn encode_decode_rpc_resp() {
        let rpc = RespRequestVoteState {
            term: Term::from(2),
            vote_granted: true,
        };

        let mut slice = vec![0; 30];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc, _) = RespRequestVoteState::decode(d_buf).unwrap();

        assert_eq!(rpc, d_rpc);
    }
}
