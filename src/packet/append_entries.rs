use crate::{
    server::Id,
    state::{
        entry::Entry,
        log::{Idx, Term, TermIdx},
    },
};
use s2n_codec::{DecoderValue, EncoderValue};

// Type used to encode the number of entries sent over an AppendEntries RPC.
pub(crate) type EntriesLenTypeEncoding = u16;

#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub struct AppendEntries {
    //% Compliance:
    // term: leader’s term
    pub term: Term,

    //% Compliance:
    //% leaderId: so follower can redirect clients
    pub leader_id: Id,

    // The last log index that the peer should have in its log.
    //
    // Used to confirm that the peer (follower) is up to date and can accept new entries. If the
    // last log index matches, then the peer is up to date and should accept this new AppendEntries
    // RPC.
    //
    //% Compliance:
    //% prevLogIndex: index of log entry immediately preceding new ones
    //% prevLogTerm: term of prevLogIndex entry
    pub prev_log_term_idx: TermIdx,

    // The index of the highest log entry known to be committed.
    //
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

    // The `prev_log_term_idx` that the peer sent in the [AppendEntries] RPC.
    //
    // This is used to match the Response to the original RPC since it is possible to have multiple
    // RPCs in-flight due to retries.
    pub echo_prev_log_term_idx: TermIdx,

    // The number of entries stored from the [AppendEntries] RPC.
    //
    // The Follower accepts all of the entries or none of them, so this is the RPC's entries.len()
    // on success and 0 on failure.
    //
    // Together with the echoed prev this gives the Leader the peer's matchIndex:
    // `echo_prev_log_term_idx.idx + entries_cnt`. Anchoring the count to the echo means the
    // matchIndex is derived from a TermIdx the Leader has already matched to its own next_idx,
    // so it cannot drift from the RPC being acknowledged.
    pub entries_cnt: EntriesLenTypeEncoding,
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
        let (entries_cnt, mut buffer) = buffer.decode::<EntriesLenTypeEncoding>()?;
        let mut entries = Vec::with_capacity(entries_cnt.into());
        for _i in 0..entries_cnt {
            let (entry, remaining_entry_buffer) = buffer.decode()?;
            // update entry_buffer
            buffer = remaining_entry_buffer;
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
        let (echo_prev_log_term_idx, buffer) = buffer.decode()?;
        let (entries_cnt, buffer) = buffer.decode()?;

        let rpc = AppendEntriesResp {
            term,
            success,
            echo_prev_log_term_idx,
            entries_cnt,
        };
        Ok((rpc, buffer))
    }
}

impl EncoderValue for AppendEntriesResp {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.term);
        encoder.write_slice(&(self.success as u8).to_be_bytes());
        encoder.encode(&self.echo_prev_log_term_idx);
        encoder.encode(&self.entries_cnt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{macros::cast_unsafe, packet::Rpc, server::ServerId, state::log::Idx};
    use s2n_codec::{DecoderBuffer, EncoderBuffer};

    // A Raft heartbeat doesn't have entries
    #[test]
    fn encode_decode_heartbeat_rpc() {
        let rpc1 = Rpc::new_append_entry(
            Term::from(2),
            ServerId::new([1; 16]),
            TermIdx::builder()
                .with_term(Term::from(3))
                .with_idx(Idx::from(4)),
            Idx::from(4),
            vec![],
        );
        let rpc1 = cast_unsafe!(rpc1, Rpc::AppendEntry);

        let rpc2 = Rpc::new_append_entry(
            Term::from(7),
            ServerId::new([9; 16]),
            TermIdx::builder()
                .with_term(Term::from(8))
                .with_idx(Idx::from(2)),
            Idx::from(1),
            vec![],
        );
        let rpc2 = cast_unsafe!(rpc2, Rpc::AppendEntry);

        let mut slice = vec![0; 200];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc1.encode(&mut buf);
        rpc2.encode(&mut buf);

        // Decoding the second RPC only lines up if the first advanced the cursor by exactly its
        // encoded length.
        let d_buf = DecoderBuffer::new(&slice);
        let (d_rpc1, d_buf) = AppendEntries::decode(d_buf).unwrap();
        let (d_rpc2, _) = AppendEntries::decode(d_buf).unwrap();

        assert_eq!(rpc1, d_rpc1);
        assert_eq!(rpc2, d_rpc2);
    }

    #[test]
    fn encode_decode_rpc() {
        // Encode two AppendEntries (each carrying entries) back-to-back into one buffer.
        //
        // Regression test: the decoder used to size the entries region with
        // `size_of::<Entry>()` (the padded in-memory size, 16B) instead of the encoded size (9B),
        // so it advanced the cursor too far. With a single RPC in a zero-padded slice the
        // over-read landed on harmless padding and hid the bug. Packing a second RPC right after
        // the first means that over-read eats into the second RPC's bytes, misaligning it — so
        // the second decode (or the final emptiness check) fails unless the cursor advances by
        // exactly the encoded length. This also mirrors how ServerIngress decodes packets
        // back-to-back from one buffer.
        let rpc1 = Rpc::new_append_entry(
            Term::from(2),
            ServerId::new([1; 16]),
            TermIdx::builder()
                .with_term(Term::from(3))
                .with_idx(Idx::from(4)),
            Idx::from(4),
            vec![Entry::new(Term::from(2), 3), Entry::new(Term::from(5), 6)],
        );
        let rpc1 = cast_unsafe!(rpc1, Rpc::AppendEntry);

        let rpc2 = Rpc::new_append_entry(
            Term::from(7),
            ServerId::new([9; 16]),
            TermIdx::builder()
                .with_term(Term::from(8))
                .with_idx(Idx::from(2)),
            Idx::from(1),
            vec![Entry::new(Term::from(7), 42)],
        );
        let rpc2 = cast_unsafe!(rpc2, Rpc::AppendEntry);

        let mut slice = vec![0; 200];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc1.encode(&mut buf);
        rpc2.encode(&mut buf);

        // Decode from ONLY the written bytes so the final buffer must be exactly empty.
        let (written, _) = buf.split_mut();
        let d_buf = DecoderBuffer::new(written);
        let (d_rpc1, d_buf) = AppendEntries::decode(d_buf).unwrap();
        let (d_rpc2, remaining) = AppendEntries::decode(d_buf).unwrap();

        assert_eq!(rpc1, d_rpc1);
        assert_eq!(rpc2, d_rpc2);
        assert!(remaining.is_empty());
    }

    #[test]
    fn encode_decode_rpc_resp() {
        let rpc1 = AppendEntriesResp {
            term: Term::from(2),
            success: true,
            echo_prev_log_term_idx: TermIdx::builder()
                .with_term(Term::from(2))
                .with_idx(Idx::from(1)),
            entries_cnt: 3,
        };

        let rpc2 = AppendEntriesResp {
            term: Term::from(5),
            success: false,
            echo_prev_log_term_idx: TermIdx::builder()
                .with_term(Term::from(4))
                .with_idx(Idx::from(3)),
            // A rejected RPC stored no entries.
            entries_cnt: 0,
        };

        let mut slice = vec![0; 60];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc1.encode(&mut buf);
        rpc2.encode(&mut buf);

        // Decode from ONLY the written bytes so the buffer must end exactly empty.
        let (written, _) = buf.split_mut();
        let d_buf = DecoderBuffer::new(written);
        let (d_rpc1, d_buf) = AppendEntriesResp::decode(d_buf).unwrap();
        let (d_rpc2, remaining) = AppendEntriesResp::decode(d_buf).unwrap();

        assert_eq!(rpc1, d_rpc1);
        assert_eq!(rpc2, d_rpc2);
        assert!(remaining.is_empty());
    }
}
