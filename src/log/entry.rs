use crate::log::{idx::Idx, term::Term, term_idx::TermIdx};

type Command = u8;

//% Compliance:
//% each log entry stores
//% 	- state machine command
//% 	- term number
//% 	- log index: integer
#[derive(Debug)]
pub(crate) struct Entry {
    term_idx: TermIdx,
    command: Command,
}

impl Entry {
    pub fn new(idx: Idx, term: Term, command: Command) -> Self {
        Entry {
            term_idx: TermIdx::builder().with_term(term).with_idx(idx),
            command,
        }
    }
}
