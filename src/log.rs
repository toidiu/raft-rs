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
    last_term_idx: TermIdx,
}

impl Log {
    pub fn new() -> Self {
        Log {
            entries: vec![],
            last_term_idx: TermIdx::initial(),
        }
    }

    pub fn last_term_idx(&self) -> TermIdx {
        self.last_term_idx
    }
}
