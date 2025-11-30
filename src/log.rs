pub use entry::Entry;
pub use idx::Idx;
use std::cmp::Ordering;
pub use term::Term;
pub use term_idx::TermIdx;

mod entry;
mod idx;
mod term;
mod term_idx;

#[derive(Debug)]
pub struct Log {
    entries: Vec<Entry>,
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

    // Return the Idx of the last entry in the Log.
    pub(crate) fn last_idx(&self) -> Idx {
        if self.entries.is_empty() {
            Idx::initial()
        } else {
            Idx::from(self.entries.len() as u64)
        }
    }

    // Return the TermIdx of the last entry in the Log.
    pub(crate) fn last_term_idx(&self) -> TermIdx {
        let last_term = self.entries.last().map_or(Term::initial(), |e| e.term);
        let last_idx = self.last_idx();
        TermIdx::builder().with_term(last_term).with_idx(last_idx)
    }

    // Return the Term at the given Idx.
    pub(crate) fn term_at_idx(&self, idx: &Idx) -> Option<Term> {
        assert!(!idx.is_initial(), "log is empty");
        self.find_entry_at(idx).map(|e| e.term)
    }

    // Update the local Log to match the Leader's Log entry.
    //
    // If entry don't match (NoMatch), then update them to equal the Leader entry. If entry does
    // not exist, then insert the Leader entry.
    pub(crate) fn update_to_match_leaders_log(
        &mut self,
        entry: Entry,
        entry_idx: Idx,
    ) -> MatchOutcome {
        assert!(
            !entry_idx.is_initial(),
            "INITIAL_IDX is placeholder value and can't be inserted."
        );
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
    pub(crate) fn entry_matches(&self, term_idx: TermIdx) -> MatchOutcome {
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

    pub fn find_entry_at(&self, idx: &Idx) -> Option<&Entry> {
        //% Compliance:
        //% `log[]` log entries; each entry contains command for state machine, and term when entry
        //% was received by leader (first index is 1)
        if *idx == Idx::initial() {
            return None;
        }
        self.entries.get(idx.as_log_idx())
    }

    pub(crate) fn is_candidate_log_up_to_date(&mut self, rpc_term_idx: &TermIdx) -> bool {
        //% Compliance:
        //% `up-to-date`: a log is considered more up-to-date than another log if:
        //%	- compare the index and term of the last entry of A's and B's log
        let log_term_idx = self.last_term_idx();
        let term_cmp = rpc_term_idx.term.cmp(&log_term_idx.term);
        let idx_cmp = rpc_term_idx.idx.cmp(&log_term_idx.idx);
        match (term_cmp, idx_cmp) {
            //% Compliance:
            //%	- if the entries have different term: the higher term is more up-to-date
            (Ordering::Greater, _) => true,
            (Ordering::Less, _) => false,
            //% Compliance:
            //%	- if the term is the same: the longer log (higher index) is more up-to-date
            (Ordering::Equal, Ordering::Less) => false,
            (Ordering::Equal, Ordering::Equal) => true,
            (Ordering::Equal, Ordering::Greater) => true,
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub fn test_get_unchecked(&self, idx: u64) -> Entry {
        let idx = Idx::from(idx);
        self.entries.get(idx.as_log_idx()).unwrap().clone()
    }

    #[cfg(test)]
    pub fn test_len(&self) -> usize {
        self.entries.len()
    }
}

#[must_use]
pub(crate) enum MatchOutcome {
    // Log entry exists at the given index and matches.
    Match,

    // Log entry exists at the given index but doesn't match.
    NoMatch,

    // Log entry for the given index doesn't exist in the local log.
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

        let outcome = log.update_to_match_leaders_log(
            Entry {
                term: Term::from(1),
                command: 8,
            },
            Idx::from(1),
        );
        assert!(matches!(outcome, MatchOutcome::DoesntExist));

        let outcome = log.update_to_match_leaders_log(
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
        let match_outcome = log.update_to_match_leaders_log(
            Entry {
                term: Term::from(2),
                command: 8,
            },
            Idx::from(1),
        );
        assert!(matches!(match_outcome, MatchOutcome::Match));

        // doesnt match
        let no_match_outcome = log.update_to_match_leaders_log(
            Entry {
                term: Term::from(3),
                command: 8,
            },
            Idx::from(3),
        );
        assert!(matches!(no_match_outcome, MatchOutcome::NoMatch));

        // doesnt exist
        let doesnt_exist_outcome = log.update_to_match_leaders_log(
            Entry {
                term: Term::from(4),
                command: 8,
            },
            Idx::from(4),
        );
        assert!(matches!(doesnt_exist_outcome, MatchOutcome::DoesntExist));
    }

    #[test]
    fn test_log_up_to_date() {
        let mut log = Log::new();

        let t1 = Term::from(1);
        let t2 = Term::from(2);
        let t3 = Term::from(3);
        let i1 = Idx::from(1);
        let i2 = Idx::from(2);
        let i3 = Idx::from(3);
        let ti_initial = TermIdx::initial();

        // Initial IS up-to-date
        // log: []
        assert!(log.is_candidate_log_up_to_date(&ti_initial));

        // log: [ [t:1] [t:2] ]
        log.push(vec![Entry::new(t1, 8)]);
        log.push(vec![Entry::new(t2, 8)]);
        assert_eq!(log.entries.len(), 2);

        // Initial NOT up-to-date
        assert!(!log.is_candidate_log_up_to_date(&ti_initial));

        // == Equal TermIdx ==
        let term_idx_eq = TermIdx::builder().with_term(t2).with_idx(i2);
        assert!(log.is_candidate_log_up_to_date(&term_idx_eq));

        // == Different Term ==
        // term <
        let term_lt = TermIdx::builder().with_term(t1).with_idx(i2);
        assert!(!log.is_candidate_log_up_to_date(&term_lt));
        // term >
        let term_gt = TermIdx::builder().with_term(t3).with_idx(i2);
        assert!(log.is_candidate_log_up_to_date(&term_gt));

        // == Same Term ==
        // idx <
        let idx_lt = TermIdx::builder().with_term(t2).with_idx(i1);
        assert!(!log.is_candidate_log_up_to_date(&idx_lt));
        // idx >
        let idx_gt = TermIdx::builder().with_term(t2).with_idx(i3);
        assert!(log.is_candidate_log_up_to_date(&idx_gt));
    }
}
