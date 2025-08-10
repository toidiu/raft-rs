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
}

impl Rpc {
    pub fn new_request_vote(term: u64, candidate_id: ServerId, last_term_idx: TermIdx) -> Rpc {
        Rpc::RequestVote(RequestVote {
            term: Term(term),
            candidate_id,
            last_log_term_idx: last_term_idx,
        })
    }

    pub fn new_request_vote_resp(term: u64, id: ServerId, vote_granted: bool) -> Rpc {
        Rpc::RespRequestVote(RespRequestVote {
            term: Term(term),
            id,
            vote_granted,
        })
    }

    pub fn new_append_entry(leader_term: u64, prev_term_idx: TermIdx) -> Rpc {
        Rpc::AppendEntries(AppendEntries {
            term: Term(leader_term),
            prev_term_idx,
        })
    }

    pub fn new_append_entry_resp(term: u64, success: bool) -> Rpc {
        Rpc::RespAppendEntries(RespAppendEntries {
            term: Term(term),
            success,
        })
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
        let (term, buffer) = buffer.decode()?;

        match tag {
            RequestVote::TAG => {
                let (candidate_id, buffer) = buffer.decode()?;
                let (last_log_term_idx, buffer) = buffer.decode()?;

                let rpc = RequestVote {
                    term,
                    candidate_id,
                    last_log_term_idx,
                };
                Ok((Rpc::RequestVote(rpc), buffer))
            }
            RespRequestVote::TAG => {
                let (id, buffer) = buffer.decode()?;
                let (vote_granted, buffer): (u8, _) = buffer.decode()?;
                let vote_granted = vote_granted != 0;
                let rpc = RespRequestVote {
                    term,
                    id,
                    vote_granted,
                };
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
                let (success, buffer): (u8, _) = buffer.decode()?;
                let success = success != 0;

                let rpc = RespAppendEntries { term, success };
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
                encoder.encode(&inner.term);
                encoder.encode(&inner.candidate_id);
                encoder.encode(&inner.last_log_term_idx);
            }
            Rpc::RespRequestVote(inner) => {
                encoder.write_slice(&[RespRequestVote::TAG]);
                encoder.encode(&inner.term);
                encoder.encode(&inner.id);
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
                encoder.write_slice(&(inner.success as u8).to_be_bytes());
            }
        }
    }
}

// Leader election
#[derive(Debug, Clone, Copy)]
pub struct RequestVote {
    // # Compliance: Fig 2
    // term: candidate’s term
    pub term: Term,

    // # Compliance: Fig 2
    // candidateId: candidate requesting vote
    pub candidate_id: ServerId,

    // # Compliance: Fig 2
    // lastLogIndex: index of candidate’s last log entry (§5.4)
    // lastLogTerm: term of candidate’s last log entry (§5.4
    pub last_log_term_idx: TermIdx,
}

impl RequestVote {
    const TAG: u8 = 1;
}

// Leader election
#[derive(Debug, Clone, Copy)]
pub struct RespRequestVote {
    // # Compliance: Fig 2
    // term: currentTerm, for candidate to update itself
    pub term: Term,

    // id of the server granting the vote
    // FIXME: this should not be part of the RPC
    pub id: ServerId,

    // # Compliance: Fig 2
    // voteGranted: true means candidate received vote
    pub vote_granted: bool,
}

impl RespRequestVote {
    const TAG: u8 = 2;
}

#[derive(Debug, Clone, Copy)]
pub struct RespAppendEntries {
    // # Compliance: Fig 2
    // term currentTerm, for leader to update itself
    pub term: Term,

    // # Compliance: Fig 2
    // success true if follower contained entry matching prevLogIndex and prevLogTerm
    success: bool,
}

impl RespAppendEntries {
    const TAG: u8 = 4;
}

// Add entries
#[derive(Debug, Clone, Copy)]
pub struct AppendEntries {
    // # Compliance: Fig 2
    // term leader’s term
    pub term: Term,

    // # Compliance: Fig 2
    // prevLogIndex index of log entry immediately preceding new ones
    //
    // # Compliance: Fig 2
    // prevLogTerm term of prevLogIndex entry
    pub prev_term_idx: TermIdx,
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

#[cfg(test)]
mod tests {
    use crate::{
        io::{BufferIo, NetTx, ServerTx, IO_BUF_LEN},
        rpc::{Rpc, TermIdx},
        state::ServerId,
        testing::cast_unsafe,
    };
    use s2n_codec::{DecoderBuffer, DecoderValue, EncoderBuffer, EncoderValue};

    #[test]
    fn encode_decode_request_vote() {
        let (mut server_io, mut network_io) = BufferIo::split();

        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        let mut sent_rpc = Rpc::new_request_vote(0, ServerId::new(), TermIdx::new(2, 3));
        sent_rpc.encode_mut(&mut buf);
        server_io.send(buf.as_mut_slice());

        let bytes = network_io.send_to_socket().unwrap();
        let buf = DecoderBuffer::new(&bytes);
        let (recv_rpc, _buf) = Rpc::decode(buf).unwrap();
        cast_unsafe!(recv_rpc, Rpc::RequestVote);
    }

    #[test]
    fn encode_decode_request_vote_res() {
        let (mut server_io, mut network_io) = BufferIo::split();

        let rpc = Rpc::new_request_vote_resp(0, ServerId::new(), true);
        server_io.send_rpc(rpc);

        network_io.send_to_socket().unwrap();
    }

    #[test]
    fn encode_decode_append_entry() {
        let (mut server_io, mut network_io) = BufferIo::split();

        let rpc = Rpc::new_append_entry(0, TermIdx::new(8, 9));
        server_io.send_rpc(rpc);

        network_io.send_to_socket().unwrap();
    }

    #[test]
    fn encode_decode_append_entry_res() {
        let (mut server_io, mut network_io) = BufferIo::split();

        let rpc = Rpc::new_append_entry_resp(0, true);
        server_io.send_rpc(rpc);

        network_io.send_to_socket().unwrap();
    }
}
