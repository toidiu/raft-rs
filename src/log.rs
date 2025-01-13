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
    pub entries: Vec<Entry>,
}

impl Log {
    pub fn new() -> Self {
        Log { entries: vec![] }
    }

    pub fn push(&mut self, entries: Vec<Entry>) {
        for entry in entries.into_iter() {
            self.entries.push(entry);
        }
    }

    pub(super) fn prev_idx(&self) -> Idx {
        Idx::from(self.entries.len() as u64)
    }

    pub fn next_idx(&self) -> Idx {
        self.prev_idx() + 1
    }

    pub fn last_term(&self) -> Term {
        self.entries.last().map_or(Term::initial(), |e| e.term)
    }

    pub fn term_at_idx(&self, idx: &Idx) -> Option<Term> {
        assert!(!idx.is_initial(), "log is empty");
        self.find_entry_at(idx).map(|e| e.term)
    }

    // Attempt to match the leader's log.
    pub fn match_leaders_log(&mut self, entry: Entry, entry_idx: Idx) -> MatchOutcome {
        assert!(!entry_idx.is_initial());
        let entry_term_idx = TermIdx::builder().with_term(entry.term).with_idx(entry_idx);

        match self.entry_matches(entry_term_idx) {
            outcome @ MatchOutcome::NoMatch => {
                //% Compliance:
                //% If an existing entry conflicts with a new one (same index but different terms),
                //% delete the existing entry and all that follow it (§5.3)
                self.entries.truncate(entry_idx.as_log_idx());
                //% Compliance:
                //% Append any new entries not already in the log
                self.entries.push(entry);

                outcome
            }
            outcome @ MatchOutcome::DoesntExist => {
                // Confirm that atleast entry_idx - 1 are present.
                //
                // To maintain the monotonically increasing property for Idx, confirm that
                // either the log is empty or entries[entry_idx - 1] exists.
                let fn_entry_min_1_exists =
                    || self.entries.get((entry_idx - 1).as_log_idx()).is_some();
                assert!(self.entries.is_empty() || fn_entry_min_1_exists());

                self.entries.push(entry);
                outcome
            }
            outcome @ MatchOutcome::Match => outcome,
        }
    }

    //% Compliance:
    //% if two entries in different logs have the same index/term, they store the same command
    pub fn entry_matches(&self, term_idx: TermIdx) -> MatchOutcome {
        // TermIdx::initial indicates that both logs are empty
        if term_idx.is_initial() && self.entries.is_empty() {
            return MatchOutcome::Match;
        }

        if let Some(entry) = self.find_entry_at(&term_idx.idx) {
            if entry.term == term_idx.term {
                MatchOutcome::Match
            } else {
                MatchOutcome::NoMatch
            }
        } else {
            MatchOutcome::DoesntExist
        }
        // entry.is_some_and(|entry| entry.term == term_idx.term)
    }

    fn find_entry_at(&self, idx: &Idx) -> Option<&Entry> {
        //% Compliance:
        //% `log[]` log entries; each entry contains command for state machine, and term when entry
        //% was received by leader (first index is 1)
        if *idx == Idx::initial() {
            return None;
        }
        self.entries.get(idx.as_log_idx())
    }
}

#[must_use]
pub enum MatchOutcome {
    Match,
    NoMatch,
    DoesntExist,
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
        assert!(log.find_entry_at(&Idx::from(1)).is_none());

        log.push(vec![entry.clone()]);

        // Find Idx::initial
        assert!(log.find_entry_at(&Idx::initial()).is_none());

        // Find existing entry
        assert_eq!(*log.find_entry_at(&Idx::from(1)).unwrap(), entry);
    }

    #[test]
    fn test_entry_matches() {
        let mut log = Log::new();
        let term = Term::from(1);
        let entry = Entry { term, command: 8 };

        // Empty log
        assert!(matches!(
            log.entry_matches(TermIdx::initial()),
            MatchOutcome::Match
        ));

        // Non-empty log
        log.push(vec![entry.clone()]);
        assert!(matches!(
            log.entry_matches(TermIdx::initial()),
            MatchOutcome::DoesntExist
        ));

        // Log entry match
        let term_idx = TermIdx::builder().with_term(term).with_idx(Idx::from(1));
        assert!(matches!(log.entry_matches(term_idx), MatchOutcome::Match));

        // Log entry mismatch
        let mis_match_term_idx = TermIdx {
            term: Term::from(2),
            idx: Idx::from(1),
        };
        assert!(matches!(
            log.entry_matches(mis_match_term_idx),
            MatchOutcome::NoMatch
        ));

        // Non-existing entry
        let non_existing_term_idx = TermIdx {
            term: Term::from(2),
            idx: Idx::from(2),
        };
        assert!(matches!(
            log.entry_matches(non_existing_term_idx),
            MatchOutcome::DoesntExist
        ));
    }

    #[should_panic]
    #[test]
    fn invalid_term_at_idx() {
        let log = Log::new();
        log.term_at_idx(&Idx::initial());
    }

    #[test]
    fn valid_term_at_idx() {
        let mut log = Log::new();
        let expected_term = Term::from(1);
        let entry = Entry {
            term: expected_term,
            command: 8,
        };
        log.push(vec![entry]);

        let term = log.term_at_idx(&Idx::from(1)).unwrap();
        assert_eq!(expected_term, term);
    }

    #[test]
    pub fn match_leaders_log_for_empty_logs() {
        let mut log = Log::new();

        let outcome = log.match_leaders_log(
            Entry {
                term: Term::from(1),
                command: 8,
            },
            Idx::from(1),
        );
        assert!(matches!(outcome, MatchOutcome::DoesntExist));

        let outcome = log.match_leaders_log(
            Entry {
                term: Term::from(1),
                command: 8,
            },
            Idx::from(2),
        );
        assert!(matches!(outcome, MatchOutcome::DoesntExist));
    }

    #[test]
    pub fn test_match_leaders_log() {
        let mut log = Log::new();
        log.push(vec![
            // Idx 1
            Entry {
                term: Term::from(2),
                command: 8,
            },
            // Idx 2
            Entry {
                term: Term::from(2),
                command: 8,
            },
            // Idx 3
            Entry {
                term: Term::from(4),
                command: 8,
            },
        ]);

        // matches
        let match_outcome = log.match_leaders_log(
            Entry {
                term: Term::from(2),
                command: 8,
            },
            Idx::from(1),
        );
        assert!(matches!(match_outcome, MatchOutcome::Match));

        // doesnt match
        let no_match_outcome = log.match_leaders_log(
            Entry {
                term: Term::from(3),
                command: 8,
            },
            Idx::from(3),
        );
        assert!(matches!(no_match_outcome, MatchOutcome::NoMatch));

        // doesnt exist
        let doesnt_exist_outcome = log.match_leaders_log(
            Entry {
                term: Term::from(4),
                command: 8,
            },
            Idx::from(4),
        );
        assert!(matches!(doesnt_exist_outcome, MatchOutcome::DoesntExist));
    }
}
