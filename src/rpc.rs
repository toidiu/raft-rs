use crate::{
    log::{Entry, Idx, Term, TermIdx},
    server::ServerId,
};
use s2n_codec::{DecoderBuffer, DecoderBufferResult, DecoderError, DecoderValue, EncoderValue};

mod append_entries;
mod request_vote;

pub use append_entries::{AppendEntriesState, RespAppendEntriesState};
pub use request_vote::{RequestVoteState, RespRequestVoteState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rpc {
    RequestVote(RequestVoteState),
    RespRequestVote(RespRequestVoteState),
    AppendEntries(AppendEntriesState),
    RespAppendEntries(RespAppendEntriesState),
}

impl Rpc {
    pub fn new_request_vote(term: Term, candidate_id: ServerId, last_log_term_idx: TermIdx) -> Rpc {
        Rpc::RequestVote(RequestVoteState {
            term,
            candidate_id,
            last_log_term_idx,
        })
    }

    pub fn new_request_vote_resp(term: Term, vote_granted: bool) -> Rpc {
        Rpc::RespRequestVote(RespRequestVoteState { term, vote_granted })
    }

    pub fn new_append_entry(
        term: Term,
        leader_id: ServerId,
        prev_log_term_idx: TermIdx,
        leader_commit_idx: Idx,
        entries: Vec<Entry>,
    ) -> Rpc {
        Rpc::AppendEntries(AppendEntriesState {
            term,
            leader_id,
            prev_log_term_idx,
            leader_commit_idx,
            entries,
        })
    }

    pub fn new_append_entry_resp(term: Term, success: bool) -> Rpc {
        Rpc::RespAppendEntries(RespAppendEntriesState { term, success })
    }

    pub fn term(&self) -> &Term {
        match self {
            Rpc::RequestVote(RequestVoteState { term, .. }) => term,
            Rpc::RespRequestVote(RespRequestVoteState { term, .. }) => term,
            Rpc::AppendEntries(AppendEntriesState { term, .. }) => term,
            Rpc::RespAppendEntries(RespAppendEntriesState { term, .. }) => term,
        }
    }
}

impl<'a> DecoderValue<'a> for Rpc {
    fn decode(buffer: DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        let (tag, buffer): (u8, _) = buffer.decode()?;

        match tag {
            RequestVoteState::TAG => {
                let (rpc, buffer) = buffer.decode()?;
                Ok((Rpc::RequestVote(rpc), buffer))
            }
            RespRequestVoteState::TAG => {
                let (rpc, buffer) = buffer.decode()?;
                Ok((Rpc::RespRequestVote(rpc), buffer))
            }
            AppendEntriesState::TAG => {
                let (rpc, buffer) = buffer.decode()?;
                Ok((Rpc::AppendEntries(rpc), buffer))
            }
            RespAppendEntriesState::TAG => {
                let (rpc, buffer) = buffer.decode()?;
                Ok((Rpc::RespAppendEntries(rpc), buffer))
            }
            _tag => Err(DecoderError::InvariantViolation("received unexpected tag")),
        }
    }
}

impl EncoderValue for Rpc {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        match self {
            Rpc::RequestVote(inner) => {
                encoder.write_slice(&[RequestVoteState::TAG]);
                encoder.encode(inner);
            }
            Rpc::RespRequestVote(inner) => {
                encoder.write_slice(&[RespRequestVoteState::TAG]);
                encoder.encode(inner);
            }
            Rpc::AppendEntries(inner) => {
                encoder.write_slice(&[AppendEntriesState::TAG]);
                encoder.encode(inner);
            }
            Rpc::RespAppendEntries(inner) => {
                encoder.write_slice(&[RespAppendEntriesState::TAG]);
                encoder.encode(inner);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use s2n_codec::EncoderBuffer;

    #[test]
    fn encode_decode_request_vote() {
        let rpc = Rpc::new_request_vote(
            Term::from(1),
            ServerId::new([10; 16]),
            TermIdx::builder()
                .with_term(Term::from(3))
                .with_idx(Idx::from(4)),
        );

        let mut slice = vec![0; 50];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc, _) = Rpc::decode(d_buf).unwrap();

        assert_eq!(rpc, d_rpc);
    }

    #[test]
    fn encode_decode_request_vote_resp() {
        let rpc = Rpc::new_request_vote_resp(Term::from(1), true);

        let mut slice = vec![0; 50];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc, _) = Rpc::decode(d_buf).unwrap();

        assert_eq!(rpc, d_rpc);
    }

    #[test]
    fn encode_decode_append_entry() {
        let rpc = Rpc::new_append_entry(
            Term::from(1),
            ServerId::new([4; 16]),
            TermIdx::builder()
                .with_term(Term::from(3))
                .with_idx(Idx::from(4)),
            Idx::from(4),
            vec![
                Entry::new(Idx::from(1), Term::from(2), 3),
                Entry::new(Idx::from(4), Term::from(5), 6),
            ],
        );

        let mut slice = vec![0; 200];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc, _) = Rpc::decode(d_buf).unwrap();

        assert_eq!(rpc, d_rpc);
    }

    #[test]
    fn encode_decode_append_entry_resp() {
        let rpc = Rpc::new_append_entry_resp(Term::from(1), true);

        let mut slice = vec![0; 50];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc, _) = Rpc::decode(d_buf).unwrap();

        assert_eq!(rpc, d_rpc);
    }
}
