use crate::{
    log::{Term, TermIdx},
    rpc::{
        append_entries::{AppendEntries, RespAppendEntries},
        request_vote::{RequestVote, RespRequestVote},
    },
    server::ServerId,
};
use s2n_codec::{DecoderBuffer, DecoderBufferResult, DecoderError, DecoderValue, EncoderValue};

mod append_entries;
mod request_vote;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rpc {
    RequestVote(RequestVote),
    RespRequestVote(RespRequestVote),
    AppendEntries(AppendEntries),
    RespAppendEntries(RespAppendEntries),
}

impl Rpc {
    pub fn new_request_vote(term: Term, candidate_id: ServerId, last_log_term_idx: TermIdx) -> Rpc {
        Rpc::RequestVote(RequestVote {
            term,
            candidate_id,
            last_log_term_idx,
        })
    }

    pub fn new_request_vote_resp(term: Term, vote_granted: bool) -> Rpc {
        Rpc::RespRequestVote(RespRequestVote { term, vote_granted })
    }

    pub fn new_append_entry(term: Term, leader_id: ServerId, prev_log_term_idx: TermIdx) -> Rpc {
        Rpc::AppendEntries(AppendEntries {
            term,
            leader_id,
            prev_log_term_idx,
        })
    }

    pub fn new_append_entry_resp(term: Term, success: bool) -> Rpc {
        Rpc::RespAppendEntries(RespAppendEntries { term, success })
    }

    pub fn term(&self) -> &Term {
        match self {
            Rpc::RequestVote(RequestVote { term, .. }) => term,
            Rpc::RespRequestVote(RespRequestVote { term, .. }) => term,
            Rpc::AppendEntries(AppendEntries { term, .. }) => term,
            Rpc::RespAppendEntries(RespAppendEntries { term, .. }) => term,
        }
    }
}

impl<'a> DecoderValue<'a> for Rpc {
    fn decode(buffer: DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        let (tag, buffer): (u8, _) = buffer.decode()?;

        match tag {
            RequestVote::TAG => {
                let (rpc, buffer) = buffer.decode()?;
                Ok((Rpc::RequestVote(rpc), buffer))
            }
            RespRequestVote::TAG => {
                let (rpc, buffer) = buffer.decode()?;
                Ok((Rpc::RespRequestVote(rpc), buffer))
            }
            AppendEntries::TAG => {
                let (rpc, buffer) = buffer.decode()?;
                Ok((Rpc::AppendEntries(rpc), buffer))
            }
            RespAppendEntries::TAG => {
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
                encoder.write_slice(&[RequestVote::TAG]);
                encoder.encode(inner);
            }
            Rpc::RespRequestVote(inner) => {
                encoder.write_slice(&[RespRequestVote::TAG]);
                encoder.encode(inner);
            }
            Rpc::AppendEntries(inner) => {
                encoder.write_slice(&[AppendEntries::TAG]);
                encoder.encode(inner);
            }
            Rpc::RespAppendEntries(inner) => {
                encoder.write_slice(&[RespAppendEntries::TAG]);
                encoder.encode(inner);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        log::Idx,
        rpc::{Rpc, Term, TermIdx},
        server::ServerId,
    };
    use s2n_codec::{DecoderBuffer, DecoderValue, EncoderBuffer, EncoderValue};

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
    fn encode_decode_request_vote_res() {
        let rpc = Rpc::new_append_entry(
            Term::from(1),
            ServerId::new([4; 16]),
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
    fn encode_decode_append_entry() {}

    #[test]
    fn encode_decode_append_entry_res() {
        let rpc = Rpc::new_append_entry_resp(Term::from(1), true);

        let mut slice = vec![0; 50];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc, _) = Rpc::decode(d_buf).unwrap();

        assert_eq!(rpc, d_rpc);
    }
}
