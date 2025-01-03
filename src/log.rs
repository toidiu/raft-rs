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
    last_committed_entry: Idx,
}

impl Log {
    pub fn new() -> Self {
        Log {
            entries: vec![],
            last_committed_entry: Idx::initial(),
        }
    }

    fn push_entry(&mut self, _entries: Vec<Entry>) {
        todo!()
    }

    fn commit_entry(&mut self, _term_idx: TermIdx) {
        todo!()
    }

    // Delete all entry >= `delete_idx` and overwrite with `new_entries`.
    //
    //% Compliance:
    //% - to make the leader/follower logs consistent:
    //% 	- the leader finds the last log entry that are the same
    //% 	- follower deletes any entries after that point
    //% 	- leader sends the follower all its entries after the common point
    //
    fn overwite_entries_at_idx(&mut self, _delete_idx: Idx, _new_entries: Vec<Entry>) {
        todo!()
    }
}
