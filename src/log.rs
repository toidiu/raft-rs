mod entry;
mod idx;
mod term;
mod term_idx;

pub use entry::Entry;
pub use idx::Idx;
pub use term::Term;
pub use term_idx::TermIdx;

#[derive(Debug)]
pub struct Log {
    entries: Vec<Entry>,
}

impl Log {
    pub fn new() -> Self {
        Log { entries: vec![] }
    }

    fn push(&mut self, entries: Vec<Entry>) {
        for entry in entries.into_iter() {
            self.entries.push(entry);
        }
    }

    pub fn next_idx(&self) -> Idx {
        let next_idx = self.entries.len() + 1;
        Idx::from(next_idx as u64)
    }

    pub fn last_term(&self) -> Term {
        self.entries.last().map_or(Term::initial(), |e| e.term)
    }

    //% Compliance:
    //% If an existing entry conflicts with a new one (same index but different terms),
    //% delete the existing entry and all that follow it (§5.3)
    pub fn match_or_delete_trailing(&mut self, entry: Entry, entry_term_idx: TermIdx) {
        let TermIdx { term: _, idx } = entry_term_idx;
        if !self.entry_matches(entry_term_idx) {
            // remove entry at current idx and all trailing ones
            self.entries.truncate(idx.into_log_idx());
            // insert the new entry
            self.entries.push(entry);
        }
    }

    //% Compliance:
    //% if two entries in different logs have the same index/term, they store the same command
    pub fn entry_matches(&self, term_idx: TermIdx) -> bool {
        // TermIdx::initial indicates that both logs are empty
        if term_idx == TermIdx::initial() {
            return self.entries.is_empty();
        }

        let entry = self.find_entry_by(term_idx.idx);
        entry.is_some_and(|entry| entry.term == term_idx.term)
    }

    fn find_entry_by(&self, idx: Idx) -> Option<&Entry> {
        //% Compliance:
        //% `log[]` log entries; each entry contains command for state machine, and term when entry
        //% was received by leader (first index is 1)
        if idx == Idx::initial() {
            return None;
        }
        self.entries.get(idx.into_log_idx())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_log_entry() {
        let mut log = Log::new();
        let entry = Entry {
            term: Term::from(1),
            command: 8,
        };

        // Empty log
        assert!(log.find_entry_by(Idx::from(1)).is_none());

        log.push(vec![entry.clone()]);

        // Find Idx::initial
        assert!(log.find_entry_by(Idx::initial()).is_none());

        // Find existing entry
        assert_eq!(*log.find_entry_by(Idx::from(1)).unwrap(), entry);
    }

    #[test]
    fn test_log_matches_at_idx() {
        let mut log = Log::new();
        let term = Term::from(1);
        let entry = Entry { term, command: 8 };

        // Empty log
        assert!(log.entry_matches(TermIdx::initial()));

        // Non-empty log
        log.push(vec![entry.clone()]);
        assert!(!log.entry_matches(TermIdx::initial()));

        // Log entry match
        let term_idx = TermIdx::builder().with_term(term).with_idx(Idx::from(1));
        assert!(log.entry_matches(term_idx));

        // Log entry mismatch
        let mis_match_term_idx = TermIdx {
            term: Term::from(2),
            idx: Idx::from(1),
        };
        assert!(!log.entry_matches(mis_match_term_idx));
    }
}
