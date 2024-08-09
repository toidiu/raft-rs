use crate::log::Term;
use bytes::{Buf, BufMut, Bytes, BytesMut};

#[derive(Debug, Clone, Copy)]
pub enum Rpc {
    RequestVote(RequestVote),
    AppendEntries(AppendEntries),
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
}

impl RequestVote {
    const TAG: u8 = 0;
}

// Add entries and heartbeat
#[derive(Debug, Clone, Copy)]
pub struct AppendEntries {
    pub term: Term,
}

impl AppendEntries {
    const TAG: u8 = 1;
}
