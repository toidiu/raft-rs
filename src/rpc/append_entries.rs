use crate::{
    log::{Entry, Idx, Term, TermIdx},
    server::ServerId,
};
use s2n_codec::{DecoderValue, EncoderValue};

// Type used to encode the number of entries sent over an AppendEntries RPC.
type EntriesLenTypeEncoding = u16;

#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub struct AppendEntries {
    //% Compliance:
    // term: leader’s term
    pub term: Term,

    //% Compliance:
    //% leaderId: so follower can redirect clients
    pub leader_id: ServerId,

    //% Compliance:
    //% prevLogIndex: index of log entry immediately preceding new ones
    //% prevLogTerm: term of prevLogIndex entry
    pub prev_log_term_idx: TermIdx,
    //% Compliance:
    // leaderCommit: leader’s commitIndex
    pub leader_commit_idx: Idx,
    //% Compliance:
    //% entries[]: log entries to store (empty for heartbeat; may send more than one for
    //% efficiency)
    pub entries: Vec<Entry>,
}

impl AppendEntries {
    pub const TAG: u8 = 3;

    pub fn term(&self) -> Term {
        self.term
    }
}

#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub struct AppendEntriesResp {
    //% Compliance:
    //% term: currentTerm, for leader to update itself
    pub term: Term,

    //% Compliance:
    //% success: true if follower contained entry matching prevLogIndex and prevLogTerm
    pub success: bool,
}

impl AppendEntriesResp {
    pub const TAG: u8 = 4;
}

impl<'a> DecoderValue<'a> for AppendEntries {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> s2n_codec::DecoderBufferResult<'a, Self> {
        let (term, buffer) = buffer.decode()?;
        let (leader_id, buffer) = buffer.decode()?;
        let (prev_log_term_idx, buffer) = buffer.decode()?;
        let (leader_commit_idx, buffer) = buffer.decode()?;

        // decode a vec of Entries
        let (entries_cnt, buffer) = buffer.decode::<EntriesLenTypeEncoding>()?;
        let entries_total_bytes = entries_cnt as usize * std::mem::size_of::<Entry>();
        let (mut entry_buffer, buffer) = buffer.decode_slice(entries_total_bytes)?;
        let mut entries = Vec::with_capacity(entries_cnt.into());
        for _i in 0..entries_cnt {
            let (entry, remaining_entry_buffer) = entry_buffer.decode()?;
            // update entry_buffer
            entry_buffer = remaining_entry_buffer;
            entries.push(entry);
        }

        let rpc = AppendEntries {
            term,
            leader_id,
            prev_log_term_idx,
            leader_commit_idx,
            entries,
        };
        Ok((rpc, buffer))
    }
}

impl EncoderValue for AppendEntries {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.term);
        encoder.encode(&self.leader_id);
        encoder.encode(&self.prev_log_term_idx);
        encoder.encode(&self.leader_commit_idx);

        // encode a vec of Entries
        //
        // Encoding representation:
        // [ entries_cnt, Vec<Entries> ]
        // [ 3, Entry, Entry, Entry ]
        let entries_cnt = self.entries.len() as EntriesLenTypeEncoding;
        encoder.encode(&(entries_cnt));
        for entry in self.entries.iter() {
            encoder.encode(entry);
        }
    }
}

impl<'a> DecoderValue<'a> for AppendEntriesResp {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> s2n_codec::DecoderBufferResult<'a, Self> {
        let (term, buffer) = buffer.decode()?;
        let (success, buffer): (u8, _) = buffer.decode()?;
        let success = success != 0;

        let rpc = AppendEntriesResp { term, success };
        Ok((rpc, buffer))
    }
}

impl EncoderValue for AppendEntriesResp {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.term);
        encoder.write_slice(&(self.success as u8).to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::Idx;
    use s2n_codec::{DecoderBuffer, EncoderBuffer};

    // A Raft heartbeat doesn't have entries
    #[test]
    fn encode_decode_heartbeat_rpc() {
        let rpc = AppendEntries {
            term: Term::from(2),
            leader_id: ServerId::new([10; 16]),
            prev_log_term_idx: TermIdx::builder()
                .with_term(Term::from(3))
                .with_idx(Idx::from(4)),
            leader_commit_idx: Idx::from(4),
            entries: vec![],
        };

        let mut slice = vec![0; 200];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc, _) = AppendEntries::decode(d_buf).unwrap();

        assert_eq!(rpc, d_rpc);
    }

    #[test]
    fn encode_decode_rpc() {
        let rpc = AppendEntries {
            term: Term::from(2),
            leader_id: ServerId::new([10; 16]),
            prev_log_term_idx: TermIdx::builder()
                .with_term(Term::from(3))
                .with_idx(Idx::from(4)),
            leader_commit_idx: Idx::from(4),
            entries: vec![Entry::new(Term::from(2), 3), Entry::new(Term::from(5), 6)],
        };

        let mut slice = vec![0; 200];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc, _) = AppendEntries::decode(d_buf).unwrap();

        assert_eq!(rpc, d_rpc);
    }

    #[test]
    fn encode_decode_rpc_resp() {
        let rpc = AppendEntriesResp {
            term: Term::from(2),
            success: true,
        };

        let mut slice = vec![0; 30];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc, _) = AppendEntriesResp::decode(d_buf).unwrap();

        assert_eq!(rpc, d_rpc);
    }
}
