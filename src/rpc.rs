use crate::{
    log::{Term, TermIdx},
    state::ServerId,
};
use s2n_codec::{DecoderBuffer, DecoderBufferResult, DecoderError, DecoderValue, EncoderValue};

#[derive(Debug, Clone, Copy)]
pub enum Rpc {
    RequestVote(RequestVote),
    RespRequestVote(RespRequestVote),
    AppendEntries(AppendEntries),
    RespAppendEntries(RespAppendEntries),
    Heartbeat(Heartbeat),
}

impl Rpc {
    pub fn new_request_vote(term: u64, candidate_id: ServerId, last_term_idx: TermIdx) -> Rpc {
        Rpc::RequestVote(RequestVote {
            term: Term(term),
            candidate_id,
            last_log_term_idx: last_term_idx,
        })
    }

    pub fn new_request_vote_resp(term: u64, vote_granted: bool) -> Rpc {
        Rpc::RespRequestVote(RespRequestVote {
            term: Term(term),
            vote_granted,
        })
    }

    pub fn new_append_entry(leader_term: u64, prev_term_idx: TermIdx) -> Rpc {
        Rpc::AppendEntries(AppendEntries {
            term: Term(leader_term),
            prev_term_idx,
        })
    }

    pub fn new_append_entry_resp(term: u64) -> Rpc {
        Rpc::RespAppendEntries(RespAppendEntries { term: Term(term) })
    }

    pub fn new_heartbeat(term: u64) -> Rpc {
        Rpc::Heartbeat(Heartbeat { term: Term(term) })
    }

    pub fn term(&self) -> &Term {
        match self {
            Rpc::RequestVote(RequestVote { term, .. }) => term,
            Rpc::RespRequestVote(RespRequestVote { term, .. }) => term,
            Rpc::AppendEntries(AppendEntries { term, .. }) => term,
            Rpc::RespAppendEntries(RespAppendEntries { term, .. }) => term,
            Rpc::Heartbeat(Heartbeat { term, .. }) => term,
        }
    }
}

impl<'a> DecoderValue<'a> for Rpc {
    fn decode(buffer: DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        let (tag, buffer): (u8, _) = buffer.decode()?;
        let (term, buffer) = buffer.decode()?;

        match tag {
            RequestVote::TAG => {
                let (candidate_id, buffer) = buffer.decode_slice(16)?;
                let candidate_id = candidate_id
                    .into_less_safe_slice()
                    .try_into()
                    .expect("already decoded 16 bytes");
                let candidate_id = ServerId(candidate_id);
                let (last_log_term_idx, buffer) = buffer.decode()?;

                let rpc = RequestVote {
                    term,
                    candidate_id,
                    last_log_term_idx,
                };
                Ok((Rpc::RequestVote(rpc), buffer))
            }
            RespRequestVote::TAG => {
                let (vote_granted, buffer): (u8, _) = buffer.decode()?;
                let vote_granted = vote_granted != 0;
                let rpc = RespRequestVote { term, vote_granted };
                Ok((Rpc::RespRequestVote(rpc), buffer))
            }
            AppendEntries::TAG => {
                let (prev_term_idx, buffer): (TermIdx, _) = buffer.decode()?;
                let rpc = AppendEntries {
                    term,
                    prev_term_idx,
                };
                Ok((Rpc::AppendEntries(rpc), buffer))
            }
            RespAppendEntries::TAG => {
                let rpc = RespAppendEntries { term };
                Ok((Rpc::RespAppendEntries(rpc), buffer))
            }
            Heartbeat::TAG => {
                let rpc = Heartbeat { term };
                Ok((Rpc::Heartbeat(rpc), buffer))
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
                encoder.encode(&inner.term);
                encoder.write_slice(&inner.candidate_id.0);
                encoder.encode(&inner.last_log_term_idx);
            }
            Rpc::RespRequestVote(inner) => {
                encoder.write_slice(&[RespRequestVote::TAG]);
                encoder.encode(&inner.term);
                encoder.write_slice(&(inner.vote_granted as u8).to_be_bytes());
            }
            Rpc::AppendEntries(inner) => {
                encoder.write_slice(&[AppendEntries::TAG]);
                encoder.encode(&inner.term);
                encoder.encode(&inner.prev_term_idx);
            }
            Rpc::RespAppendEntries(inner) => {
                encoder.write_slice(&[RespAppendEntries::TAG]);
                encoder.encode(&inner.term);
            }
            Rpc::Heartbeat(inner) => {
                encoder.write_slice(&[Heartbeat::TAG]);
                encoder.encode(&inner.term);
            }
        }
    }
}

// Leader election
#[derive(Debug, Clone, Copy)]
pub struct RequestVote {
    pub term: Term,
    pub candidate_id: ServerId,
    pub last_log_term_idx: TermIdx,
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

#[derive(Debug, Clone, Copy)]
pub struct RespAppendEntries {
    pub term: Term,
}

impl RespAppendEntries {
    const TAG: u8 = 4;
}

// Add entries
#[derive(Debug, Clone, Copy)]
pub struct AppendEntries {
    // # Compliance: Fig 2
    // term leader’s term
    term: Term,
    // # Compliance: Fig 2
    // prevLogIndex index of log entry immediately preceding
    // new ones
    //
    // # Compliance: Fig 2
    // prevLogTerm term of prevLogIndex entry
    prev_term_idx: TermIdx,
    //
    // # Compliance: Fig 2
    // TODO leaderId so follower can redirect clients
    //
    // # Compliance: Fig 2
    // TODO entries[] log entries to store (empty for heartbeat; may send more than one for
    // efficiency)
    //
    // # Compliance: Fig 2
    // TODO leaderCommit leader’s commitIndex
}

impl AppendEntries {
    const TAG: u8 = 3;

    pub fn term(&self) -> Term {
        self.term
    }
}

// Heartbeat.
//
// Dedicated message since it doesn't need to be committed nor a response.
#[derive(Debug, Clone, Copy)]
pub struct Heartbeat {
    pub term: Term,
}

impl Heartbeat {
    const TAG: u8 = 5;
}

#[cfg(test)]
mod tests {
    use crate::{
        io::{BufferIo, NetTx, ServerTx, IO_BUF_LEN},
        rpc::{Rpc, TermIdx},
        state::ServerId,
    };
    use s2n_codec::{EncoderBuffer, EncoderValue};

    #[test]
    fn encode_decode_heartbeat() {
        let (mut server_io, mut network_io) = BufferIo::split();

        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_heartbeat(1).encode_mut(&mut buf);
        server_io.send(buf.as_mut_slice().to_vec());

        network_io.send().unwrap();
    }

    #[test]
    fn encode_decode_request_vote() {
        let (mut server_io, mut network_io) = BufferIo::split();

        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_request_vote(0, ServerId::new(), TermIdx::new(2, 3)).encode_mut(&mut buf);
        server_io.send(buf.as_mut_slice().to_vec());

        network_io.send().unwrap();
    }

    #[test]
    fn encode_decode_request_vote_res() {
        let (mut server_io, mut network_io) = BufferIo::split();

        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_request_vote_resp(0, true).encode_mut(&mut buf);
        server_io.send(buf.as_mut_slice().to_vec());

        network_io.send().unwrap();
    }

    #[test]
    fn encode_decode_append_entry() {
        let (mut server_io, mut network_io) = BufferIo::split();

        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_append_entry(0, TermIdx::new(8, 9)).encode_mut(&mut buf);
        server_io.send(buf.as_mut_slice().to_vec());

        network_io.send().unwrap();
    }

    #[test]
    fn encode_decode_append_entry_res() {
        let (mut server_io, mut network_io) = BufferIo::split();

        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        Rpc::new_append_entry_resp(0).encode_mut(&mut buf);
        server_io.send(buf.as_mut_slice().to_vec());

        network_io.send().unwrap();
    }
}
