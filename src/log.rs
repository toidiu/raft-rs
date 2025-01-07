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
    pub fn log_matches_at_idx(&self, term_idx: TermIdx) -> bool {
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
