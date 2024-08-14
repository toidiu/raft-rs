use crate::log::Term;
use bytes::{Buf, BufMut, Bytes, BytesMut};

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

impl TryFrom<Bytes> for Rpc {
    type Error = ();

    fn try_from(mut value: Bytes) -> std::result::Result<Rpc, ()> {
        let tag = value.get_u8();
        match tag {
            RequestVote::TAG => {
                let term = value.get_u64();
                let rpc = RequestVote { term: Term(term) };
                Ok(Rpc::RequestVote(rpc))
            }
            RespRequestVote::TAG => {
                let term = value.get_u64();
                let vote_granted = value.get_u8() != 0;
                let rpc = RespRequestVote {
                    term: Term(term),
                    vote_granted,
                };
                Ok(Rpc::RespRequestVote(rpc))
            }
            AppendEntries::TAG => {
                let term = value.get_u64();
                let rpc = AppendEntries { term: Term(term) };
                Ok(Rpc::AppendEntries(rpc))
            }
            _ => Err(()),
        }
    }
}

impl From<Rpc> for Bytes {
    fn from(value: Rpc) -> Self {
        let mut b = BytesMut::with_capacity(10);
        let b = match value {
            Rpc::RequestVote(inner) => {
                b.put_u8(RequestVote::TAG);
                b.put_u64(inner.term.0);
                b
            }
            Rpc::RespRequestVote(inner) => {
                b.put_u8(RespRequestVote::TAG);
                b.put_u64(inner.term.0);
                b.put_u8(inner.vote_granted as u8);
                b
            }
            Rpc::AppendEntries(inner) => {
                b.put_u8(AppendEntries::TAG);
                b.put_u64(inner.term.0);
                b
            }
        };

        b.freeze()
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
    const TAG: u8 = 0;
}

// Leader election
#[derive(Debug, Clone, Copy)]
pub struct RespRequestVote {
    pub term: Term,
    pub vote_granted: bool,
}

impl RespRequestVote {
    const TAG: u8 = 1;
}

// Add entries and heartbeat
#[derive(Debug, Clone, Copy)]
pub struct AppendEntries {
    pub term: Term,
}

impl AppendEntries {
    const TAG: u8 = 2;
}
