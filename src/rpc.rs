use crate::log::Term;
use s2n_codec::{DecoderBuffer, DecoderBufferResult, DecoderError, DecoderValue, EncoderValue};

pub const TAG_LEN: usize = 1;

#[derive(Debug, Clone, Copy)]
pub enum Rpc {
    RequestVote(RequestVote),
    RespRequestVote(RespRequestVote),
    AppendEntries(AppendEntries),
    // RespAppendEntries(AppendEntries),
}

impl Rpc {
    pub fn new_request_vote(term: u64) -> Rpc {
        Rpc::RequestVote(RequestVote { term: Term(term) })
    }

    pub fn new_append_entry(term: u64) -> Rpc {
        Rpc::AppendEntries(AppendEntries { term: Term(term) })
    }
}

impl<'a> DecoderValue<'a> for Rpc {
    fn decode(buffer: DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        // println!("-------RPC decode. buffer: {:?}", buffer);
        let (tag, buffer): (u8, _) = buffer.decode().expect("todo");
        let (term, buffer): (u64, _) = buffer.decode().expect("todo");

        let rpc = match tag {
            RequestVote::TAG => {
                let rpc = RequestVote { term: term.into() };
                Ok(Rpc::RequestVote(rpc))
            }
            RespRequestVote::TAG => {
                let (vote_granted, _buffer): (u8, _) = buffer.decode().expect("todo");
                let vote_granted = vote_granted != 0;
                let rpc = RespRequestVote {
                    term: Term(term),
                    vote_granted,
                };
                Ok(Rpc::RespRequestVote(rpc))
            }
            AppendEntries::TAG => {
                let rpc = AppendEntries { term: term.into() };
                Ok(Rpc::AppendEntries(rpc))
            }
            _tag => Err(DecoderError::InvariantViolation("received unexpected tag")),
        };
        rpc.map(|rpc| (rpc, buffer))
    }
}

impl EncoderValue for Rpc {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        match self {
            Rpc::RequestVote(inner) => {
                encoder.write_slice(&[RequestVote::TAG]);
                encoder.write_slice(&inner.term.0.to_be_bytes());
            }
            Rpc::RespRequestVote(inner) => {
                encoder.write_slice(&[RespRequestVote::TAG]);
                encoder.write_slice(&inner.term.0.to_be_bytes());
                encoder.write_slice(&(inner.vote_granted as u8).to_be_bytes());
            }
            Rpc::AppendEntries(inner) => {
                encoder.write_slice(&[AppendEntries::TAG]);
                encoder.write_slice(&inner.term.0.to_be_bytes());
            }
        }
    }
}

// Leader election
#[derive(Debug, Clone, Copy)]
pub struct RequestVote {
    pub term: Term,
    // pub cnadidate_d: ServerId,
    // pub last_term_idx: TermIdx,
}

impl RequestVote {
    const TAG: u8 = 1;
}

// Leader election
#[derive(Debug, Clone, Copy)]
pub struct RespRequestVote {
    pub term: Term,
    pub vote_granted: bool,
}

impl RespRequestVote {
    const TAG: u8 = 2;
}

// Add entries and heartbeat
#[derive(Debug, Clone, Copy)]
pub struct AppendEntries {
    pub term: Term,
}

impl AppendEntries {
    const TAG: u8 = 3;
}
