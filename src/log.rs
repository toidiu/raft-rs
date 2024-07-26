use core::{cmp::Ordering, ops::AddAssign};
use s2n_codec::{DecoderBufferResult, DecoderValue, EncoderValue};

// Initial entry indicative of an empty log.
//
// Two new servers (empty logs) receiving this TermIdx value will agree on the same committed last
// TermIdx and therefore make progress.
const INITIAL_LAST_TERM_IDX: TermIdx = TermIdx::new(0, 0);

// The data type supported by this Raft implementation.
// TODO: u8 is used for simplification. Eventually support additional types.
type Data = u8;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Term(pub u64);

impl AddAssign<u64> for Term {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl From<u64> for Term {
    fn from(value: u64) -> Self {
        Term(value)
    }
}

impl<'a> DecoderValue<'a> for Term {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        let (term, buffer): (u64, _) = buffer.decode()?;
        let term = Term(term);
        Ok((term, buffer))
    }
}

impl EncoderValue for Term {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.write_slice(&self.0.to_be_bytes());
    }
}

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Idx(u64);

impl<'a> DecoderValue<'a> for Idx {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        let (idx, buffer): (u64, _) = buffer.decode()?;
        let idx = Idx(idx);
        Ok((idx, buffer))
    }
}

impl EncoderValue for Idx {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.write_slice(&self.0.to_be_bytes());
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct TermIdx {
    term: Term,
    idx: Idx,
}

impl PartialOrd for TermIdx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TermIdx {
    fn cmp(&self, other: &Self) -> Ordering {
        // compare term
        let term = self.term.cmp(&other.term);

        // compare idx
        let idx = self.idx.cmp(&other.idx);

        match (term, idx) {
            (Ordering::Less, _) => Ordering::Less,
            (Ordering::Equal, Ordering::Less) => Ordering::Less,
            (Ordering::Equal, Ordering::Equal) => Ordering::Equal,
            (Ordering::Equal, Ordering::Greater) => Ordering::Greater,
            (Ordering::Greater, _) => Ordering::Greater,
        }
    }
}

impl<'a> DecoderValue<'a> for TermIdx {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        let (term, buffer): (Term, _) = buffer.decode()?;
        let (idx, buffer): (Idx, _) = buffer.decode()?;
        Ok((TermIdx { term, idx }, buffer))
    }
}

impl EncoderValue for TermIdx {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.term);
        encoder.encode(&self.idx);
    }
}

impl TermIdx {
    pub const fn new(term: u64, idx: u64) -> Self {
        TermIdx {
            term: Term(term),
            idx: Idx(idx),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    term_idx: TermIdx,
    data: Data,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        // Since Raft ensures that only the Leader can commit entries,
        // it is sufficient to compare the Term and Idx
        self.term_idx == other.term_idx
    }
}

impl Eq for Entry {}

impl Entry {
    fn new(term: Term, idx: Idx, data: Data) -> Self {
        let term_idx = TermIdx { term, idx };
        Entry { term_idx, data }
    }
}

#[derive(Default, Debug)]
pub struct Log {
    entries: Vec<Entry>,
}

impl Log {
    pub fn new() -> Self {
        Log {
            entries: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn push(&mut self, entry: Entry) -> Result<(), ()> {
        // validation
        let last = self.last_committed_term_idx();

        // A placeholder TERM_IDX indicative of an empty log
        if last != INITIAL_LAST_TERM_IDX {
            // new entries should not have a smaller term
            debug_assert!(entry.term_idx.term >= last.term);

            // term is greater, then idx == 0
            let valid_next_term = (entry.term_idx.term > last.term) && entry.term_idx.idx == Idx(0);

            // if term is eq then idx should be +1
            let valid_curr_term = Idx(last.idx.0 + 1) == entry.term_idx.idx;

            let valid_entry = valid_curr_term || valid_next_term;

            if !valid_entry {
                // specific error. entries must be uniquely increasing
                return Err(());
            }
        };

        self.entries.push(entry);
        Ok(())
    }

    fn pop(&mut self) -> Option<Entry> {
        self.entries.pop()
    }

    // An entry is committed to the Log if the leader sends an AppendEntries to followers and get a
    // response from majority of the followers
    pub fn last_committed_term_idx(&self) -> TermIdx {
        let entry = self
            .entries
            .iter()
            .rev()
            .peekable()
            .peek()
            .map(|entry| entry.term_idx);

        entry.unwrap_or(INITIAL_LAST_TERM_IDX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // - adding new entries to Log
    // - removing entries from Log
    #[test]
    fn push_pop() {
        let mut log = Log::default();
        assert_eq!(log.len(), 0);

        let mut entry = Entry::new(Term(0), Idx(0), 1);
        log.push(entry.clone()).unwrap();
        entry.term_idx.term = Term(2);
        log.push(entry.clone()).unwrap();
        assert_eq!(log.last_committed_term_idx(), TermIdx::new(2, 0));
        entry.term_idx.term = Term(3);
        log.push(entry.clone()).unwrap();
        entry.term_idx.idx = Idx(1);
        log.push(entry.clone()).unwrap();
        assert_eq!(log.len(), 4);

        assert_eq!(log.pop().unwrap(), Entry::new(Term(3), Idx(1), 1));
        assert_eq!(log.pop().unwrap(), Entry::new(Term(3), Idx(0), 1));
        assert_eq!(log.pop().unwrap(), Entry::new(Term(2), Idx(0), 1));
        assert_eq!(log.pop().unwrap(), Entry::new(Term(0), Idx(0), 1));
        assert_eq!(log.len(), 0);
    }

    // - ensure new entries.idx are monotonically increasing
    #[test]
    fn monotonically_increasing_idx() {
        let mut log = Log::default();

        let mut e = Entry::new(Term(1), Idx(0), 1);
        log.push(e.clone()).unwrap();

        // same Idx fails
        assert!(log.push(e.clone()).is_err());

        // Idx +2 fails
        e.term_idx.idx = Idx(2);
        assert!(log.push(e.clone()).is_err());

        // greater Idx succeeds
        e.term_idx.idx = Idx(1);
        log.push(e.clone()).unwrap();

        // smaller Idx fails
        e.term_idx.idx = Idx(0);
        assert!(log.push(e.clone()).is_err());
    }

    // - ensure new entries.term are monotonically increasing
    #[test]
    fn monotonically_increasing_term() {
        let mut log = Log::default();

        let mut e = Entry::new(Term(1), Idx(0), 1);
        log.push(e.clone()).unwrap();

        // same Term fails
        assert!(log.push(e.clone()).is_err());

        // Term +1 succeeds
        e.term_idx.term = Term(2);
        log.push(e.clone()).unwrap();

        // Term +2 succeeds
        e.term_idx.term = Term(4);
        log.push(e.clone()).unwrap();
    }

    #[should_panic]
    #[test]
    fn monotonically_smaller_term() {
        let mut log = Log::default();

        let mut e = Entry::new(Term(4), Idx(0), 1);
        log.push(e.clone()).unwrap();

        // smaller Term fails
        e.term_idx.term = Term(3);
        let _ = log.push(e.clone());
    }

    // - new term idx should be 0
    #[test]
    fn new_term_idx() {
        let mut log = Log::default();

        let mut e = Entry::new(Term(1), Idx(0), 1);
        log.push(e.clone()).unwrap();

        // Term +1 fails if idx != 0
        e.term_idx.term = Term(2);
        e.term_idx.idx = Idx(2);
        assert!(log.push(e.clone()).is_err());

        // Term +1 succeeds if idx == 0
        e.term_idx.term = Term(2);
        e.term_idx.idx = Idx(0);
        log.push(e.clone()).unwrap();
    }

    // - ensure last_term_idx when we pop entries
    #[test]
    fn last_term_idx_accounting() {
        let mut log = Log::default();

        let mut e = Entry::new(Term(1), Idx(0), 1);
        log.push(e.clone()).unwrap();
        assert_eq!(log.last_committed_term_idx(), TermIdx::new(1, 0));

        // remove last_term_idx
        log.pop();
        assert_eq!(log.last_committed_term_idx(), INITIAL_LAST_TERM_IDX);

        // can push the same entry `e` since we popped it
        log.push(e.clone()).unwrap();

        // push 2 more entries
        e.term_idx.term = Term(2);
        log.push(e.clone()).unwrap();
        assert_eq!(log.last_committed_term_idx(), TermIdx::new(2, 0));
        e.term_idx.term = Term(3);
        log.push(e.clone()).unwrap();

        // ensure that last_term_idx was updated
        assert_eq!(log.pop().unwrap().term_idx, TermIdx::new(3, 0));
        assert_eq!(log.last_committed_term_idx(), TermIdx::new(2, 0));
    }

    // - get term,idx of last LogEntry
    #[test]
    fn last_entry() {
        let mut log = Log::default();
        assert_eq!(log.last_committed_term_idx(), INITIAL_LAST_TERM_IDX);

        log.push(Entry::new(Term(0), Idx(0), 1)).unwrap();
        assert_eq!(log.last_committed_term_idx(), TermIdx::new(0, 0));

        log.push(Entry::new(Term(0), Idx(1), 2)).unwrap();
        log.push(Entry::new(Term(0), Idx(2), 2)).unwrap();
        assert_eq!(log.last_committed_term_idx(), TermIdx::new(0, 2));
    }

    // - comparing log entry
    #[test]
    fn compare_entry() {
        let e1 = Entry::new(Term(0), Idx(0), 1);
        let e2 = Entry::new(Term(0), Idx(0), 1);
        assert_eq!(e1, e2);

        let e1 = Entry::new(Term(0), Idx(0), 1);
        let e2 = Entry::new(Term(0), Idx(0), 2);
        assert_eq!(e1, e2);

        let e1 = Entry::new(Term(0), Idx(0), 1);
        let e2 = Entry::new(Term(0), Idx(1), 2);
        assert!(e1 != e2);

        let e1 = Entry::new(Term(0), Idx(0), 1);
        let e2 = Entry::new(Term(1), Idx(0), 2);
        assert!(e1 != e2);
    }
}
