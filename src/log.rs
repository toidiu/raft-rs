use std::collections::VecDeque;

// The data type supported by this Raft implementation.
// TODO: u8 is used for simplification. Eventually support additional types.
pub type Data = u8;

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub struct Term(u64);

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub struct Idx(u64);

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct Entry {
    term: Term,
    idx: Idx,
    data: Data,
}

impl Entry {
    fn new(term: Term, idx: Idx, data: Data) -> Self {
        Entry { term, idx, data }
    }
}

#[derive(Default, Debug)]
pub struct Log {
    entries: VecDeque<Entry>,
}

impl Log {
    fn len(&self) -> usize {
        self.entries.len()
    }

    fn push(&mut self, entry: Entry) {
        self.entries.push_back(entry)
    }

    fn pop(&mut self) -> Option<Entry> {
        self.entries.pop_front()
    }

    fn last_term_idx(&self) -> Option<(Term, Idx)> {
        let entry = self
            .entries
            .iter()
            .rev()
            .peekable()
            .peek()
            .map(|entry| (entry.term, entry.idx));

        entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // - adding new entries to Log
    // - removing entries from Log
    #[test]
    fn log_operations() {
        let mut log = Log::default();
        assert_eq!(log.len(), 0);

        let entry = Entry::new(Term(0), Idx(0), 1);
        log.push(entry.clone());
        assert_eq!(log.len(), 1);

        assert_eq!(log.pop().unwrap(), entry);
        assert_eq!(log.len(), 0);
    }

    // - get term,idx of last LogEntry
    #[test]
    fn last_entry() {
        let mut log = Log::default();
        assert!(log.last_term_idx().is_none());

        log.push(Entry::new(Term(0), Idx(0), 1));
        assert_eq!(log.last_term_idx().unwrap(), (Term(0), Idx(0)));

        log.push(Entry::new(Term(0), Idx(1), 2));
        assert_eq!(log.last_term_idx().unwrap(), (Term(0), Idx(1)));
    }
}
