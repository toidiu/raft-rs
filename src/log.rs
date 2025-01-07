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

    pub fn last_term_idx(&self) -> TermIdx {
        self.entries
            .last()
            .map_or(TermIdx::initial(), |e| e.term_idx)
    }

    //% Compliance:
    //% if two entries in different logs have the same index/term, they store the same command
    pub fn entry_matches(&self, term_idx: TermIdx) -> bool {
        // TermIdx::initial indicates that both logs are empty
        if term_idx == TermIdx::initial() {
            return self.entries.is_empty();
        }

        let entry = self.find_log_entry(term_idx.idx);
        entry.map_or(false, |entry| entry.term_idx == term_idx)
    }

    pub fn find_log_entry(&self, idx: Idx) -> Option<&Entry> {
        //% Compliance:
        //% `log[]` log entries; each entry contains command for state machine, and term when entry
        //% was received by leader (first index is 1)
        if idx == Idx::initial() {
            return None;
        }
        let idx = (idx.0 - 1) as usize;

        self.entries.get(idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_log_entry() {
        let mut log = Log::new();
        let entry = Entry {
            term_idx: TermIdx {
                term: Term::from(1),
                idx: Idx::from(1),
            },
            command: 8,
        };
        log.entries.push(entry.clone());

        // Empty log
        assert!(log.find_log_entry(Idx::initial()).is_none());

        // Log contains entry at idx
        assert_eq!(*log.find_log_entry(Idx::from(1)).unwrap(), entry);
    }

    #[test]
    fn test_log_matches_at_idx() {
        let mut log = Log::new();
        let term_idx = TermIdx {
            term: Term::from(1),
            idx: Idx::from(1),
        };
        let entry1 = Entry {
            term_idx,
            command: 8,
        };
        log.entries.push(entry1.clone());

        // Empty log
        assert!(!log.entry_matches(TermIdx::initial()));

        // Log entry match
        assert!(log.entry_matches(term_idx));

        // Log entry mismatch
        let mis_match_term_idx = TermIdx {
            term: Term::from(2),
            idx: Idx::from(1),
        };
        assert!(!log.entry_matches(mis_match_term_idx));
    }
}
