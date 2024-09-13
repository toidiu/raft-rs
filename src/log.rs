// The data type supported by this Raft implementation.
// TODO: u8 is used for simplification. Eventually support additional types.
pub type Data = u8;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Term(pub u64);

impl From<u64> for Term {
    fn from(value: u64) -> Self {
        Term(value)
    }
}

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Idx(u64);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct TermIdx {
    term: Term,
    idx: Idx,
}

impl TermIdx {
    fn new(term: u64, idx: u64) -> Self {
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
        if let Some(last) = self.last_term_idx() {
            // if term is eq then idx should be +1
            let valid_curr_term = Idx(last.idx.0 + 1) == entry.term_idx.idx;
            // term is greater and idx == 0
            let valid_next_term = (entry.term_idx.term > last.term) && entry.term_idx.idx == Idx(0);
            let valid_entry = valid_curr_term || valid_next_term;

            if !valid_entry {
                // TODO specific error. entries must be uniquely increasing
                return Err(());
            }
        };

        self.entries.push(entry);
        Ok(())
    }

    fn pop(&mut self) -> Option<Entry> {
        self.entries.pop()
    }

    pub fn last_term_idx(&self) -> Option<TermIdx> {
        let entry = self
            .entries
            .iter()
            .rev()
            .peekable()
            .peek()
            .map(|entry| entry.term_idx);

        entry
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
        assert_eq!(log.last_term_idx().unwrap(), TermIdx::new(2, 0));
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

        // smaller Term fails
        e.term_idx.term = Term(3);
        assert!(log.push(e.clone()).is_err());
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
        assert_eq!(log.last_term_idx().unwrap(), TermIdx::new(1, 0));

        // remove last_term_idx
        log.pop();
        assert!(log.last_term_idx().is_none());

        // can push the same entry `e` since we popped it
        log.push(e.clone()).unwrap();

        // push 2 more entries
        e.term_idx.term = Term(2);
        log.push(e.clone()).unwrap();
        assert_eq!(log.last_term_idx().unwrap(), TermIdx::new(2, 0));
        e.term_idx.term = Term(3);
        log.push(e.clone()).unwrap();

        // ensure that last_term_idx was updated
        assert_eq!(log.pop().unwrap().term_idx, TermIdx::new(3, 0));
        assert_eq!(log.last_term_idx().unwrap(), TermIdx::new(2, 0));
    }

    // - get term,idx of last LogEntry
    #[test]
    fn last_entry() {
        let mut log = Log::default();
        assert!(log.last_term_idx().is_none());

        log.push(Entry::new(Term(0), Idx(0), 1)).unwrap();
        assert_eq!(log.last_term_idx().unwrap(), TermIdx::new(0, 0));

        log.push(Entry::new(Term(0), Idx(1), 2)).unwrap();
        log.push(Entry::new(Term(0), Idx(2), 2)).unwrap();
        assert_eq!(log.last_term_idx().unwrap(), TermIdx::new(0, 2));
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
