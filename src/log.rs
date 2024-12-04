use crate::term_idx::{Idx, Term, TermIdx};

// The data type supported by this Raft implementation.
// TODO: u8 is used for simplification. Eventually support additional types.
type Data = u8;

const INITIAL_IDX: u64 = 1;

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
        if let Some(last_term_idx) = self.last_committed_term_idx() {
            // new entries should not have a smaller term
            debug_assert!(entry.term_idx.term() >= last_term_idx.term());

            // term is greater, then idx == 1
            let term_greater = entry.term_idx.term() > last_term_idx.term();
            let idx_eq_1 = entry.term_idx.idx() == Idx(INITIAL_IDX);
            let valid_next_term = term_greater && idx_eq_1;

            // if term is eq then idx should be +1
            let curr_term_eq = entry.term_idx.term() == last_term_idx.term();
            let idx_one_greater = Idx(last_term_idx.idx().0 + 1) == entry.term_idx.idx();
            let valid_curr_term = curr_term_eq && idx_one_greater;

            dbg!(
                valid_curr_term,
                valid_next_term,
                last_term_idx,
                entry.term_idx
            );
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
    pub fn last_committed_term_idx(&self) -> Option<TermIdx> {
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
        let term_idx = TermIdx::new(term.0, idx.0);
        Entry { term_idx, data }
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

        let entry = Entry::new(Term(0), Idx(1), 1);
        log.push(entry).unwrap();

        let entry = Entry::new(Term(2), Idx(1), 1);
        log.push(entry).unwrap();
        assert_eq!(log.last_committed_term_idx().unwrap(), TermIdx::new(2, 1));

        let entry = Entry::new(Term(3), Idx(1), 1);
        log.push(entry).unwrap();

        let entry = Entry::new(Term(3), Idx(2), 1);
        log.push(entry).unwrap();
        assert_eq!(log.len(), 4);

        assert_eq!(log.pop().unwrap(), Entry::new(Term(3), Idx(2), 1));
        assert_eq!(log.pop().unwrap(), Entry::new(Term(3), Idx(1), 1));
        assert_eq!(log.pop().unwrap(), Entry::new(Term(2), Idx(1), 1));
        assert_eq!(log.pop().unwrap(), Entry::new(Term(0), Idx(1), 1));
        assert_eq!(log.len(), 0);
    }

    // - ensure new entries.idx are monotonically increasing
    #[test]
    fn monotonically_increasing_idx() {
        let mut log = Log::default();

        let e = Entry::new(Term(1), Idx(2), 1);
        log.push(e.clone()).unwrap();

        // same Idx fails
        assert!(log.push(e.clone()).is_err());

        // Idx +2 fails
        let e = Entry::new(Term(1), Idx(4), 1);
        assert!(log.push(e).is_err());

        // greater Idx succeeds
        let e = Entry::new(Term(1), Idx(3), 1);
        log.push(e).unwrap();

        // smaller Idx fails
        let e = Entry::new(Term(1), Idx(1), 1);
        assert!(log.push(e).is_err());
    }

    // - ensure new entries.term are monotonically increasing
    #[test]
    fn monotonically_increasing_term() {
        let mut log = Log::default();

        let e = Entry::new(Term(1), Idx(1), 1);
        log.push(e.clone()).unwrap();

        // same Term fails
        assert!(log.push(e.clone()).is_err());

        // Term +1 succeeds
        let e = Entry::new(Term(2), Idx(1), 1);
        log.push(e).unwrap();

        // Term +2 succeeds
        let e = Entry::new(Term(4), Idx(1), 1);
        log.push(e).unwrap();
    }

    #[should_panic]
    #[test]
    fn monotonically_smaller_term() {
        let mut log = Log::default();

        let e = Entry::new(Term(4), Idx(0), 1);
        log.push(e).unwrap();

        // smaller Term fails
        let e = Entry::new(Term(3), Idx(0), 1);
        let _ = log.push(e);
    }

    // - new term idx should be 0
    #[test]
    fn new_term_idx() {
        let mut log = Log::default();

        let e = Entry::new(Term(1), Idx(1), 1);
        log.push(e).unwrap();

        // Term +1 fails if idx > 1
        let e = Entry::new(Term(2), Idx(2), 1);
        assert!(log.push(e).is_err());

        // Term +1 succeeds if idx == 1
        let e = Entry::new(Term(2), Idx(1), 1);
        log.push(e).unwrap();
    }

    // - ensure last_term_idx when we pop entries
    #[test]
    fn last_term_idx_accounting() {
        let mut log = Log::default();

        let e = Entry::new(Term(1), Idx(1), 1);
        log.push(e.clone()).unwrap();
        assert_eq!(log.last_committed_term_idx().unwrap(), TermIdx::new(1, 1));

        // remove last_term_idx
        log.pop();
        assert!(log.last_committed_term_idx().is_none());

        // can push the same entry `e` since we popped it
        log.push(e.clone()).unwrap();

        // push 2 more entries
        let e = Entry::new(Term(2), Idx(1), 1);
        log.push(e).unwrap();
        assert_eq!(log.last_committed_term_idx().unwrap(), TermIdx::new(2, 1));
        let e = Entry::new(Term(3), Idx(1), 1);
        log.push(e).unwrap();

        // ensure that last_term_idx was updated
        assert_eq!(log.pop().unwrap().term_idx, TermIdx::new(3, 1));
        assert_eq!(log.last_committed_term_idx().unwrap(), TermIdx::new(2, 1));
    }

    // - get term,idx of last LogEntry
    #[test]
    fn last_entry() {
        let mut log = Log::default();
        assert!(log.last_committed_term_idx().is_none());

        log.push(Entry::new(Term(0), Idx(1), 1)).unwrap();
        assert_eq!(log.last_committed_term_idx().unwrap(), TermIdx::new(0, 1));

        log.push(Entry::new(Term(0), Idx(2), 2)).unwrap();
        log.push(Entry::new(Term(0), Idx(3), 2)).unwrap();
        assert_eq!(log.last_committed_term_idx().unwrap(), TermIdx::new(0, 3));
    }

    // - comparing entry
    #[test]
    fn compare_entry() {
        let e1 = Entry::new(Term(0), Idx(1), 1);
        let e2 = Entry::new(Term(0), Idx(1), 1);
        assert_eq!(e1, e2);

        let e1 = Entry::new(Term(0), Idx(1), 1);
        let e2 = Entry::new(Term(0), Idx(1), 2);
        assert_eq!(e1, e2);

        let e1 = Entry::new(Term(0), Idx(1), 1);
        let e2 = Entry::new(Term(0), Idx(2), 2);
        assert!(e1 != e2);

        let e1 = Entry::new(Term(0), Idx(1), 1);
        let e2 = Entry::new(Term(1), Idx(1), 2);
        assert!(e1 != e2);
    }
}
