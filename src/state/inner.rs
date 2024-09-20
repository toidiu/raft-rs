use crate::{
    clock::{Clock, Timer},
    log::{Log, TermIdx},
    state::{ServerId, Term},
};

#[derive(Debug)]
pub struct Inner {
    // # Compliance: Fig 2
    // currentTerm: latest term server has seen (initialized to 0 on first boot, increases
    // monotonically)
    pub curr_term: Term,

    // list of all raft servers
    server_list: Vec<ServerId>,

    // # Compliance: Fig 2
    // votedFor: candidateId that received vote in current
    pub voted_for: Option<ServerId>,

    // # Compliance: Fig 2
    // log[]: log entries; each entry contains command for state machine, and term when entry was
    // received by leader (first index is 1)
    log: Log,

    // TODO
    // lastApplied: index of highest log entry applied to state machine (initialized to 0, increases
    // monotonically)
    // last_applied: todo!(),
    pub timer: Timer,
}

impl Inner {
    pub fn new(clock: Clock, server_list: Vec<ServerId>) -> Self {
        Inner {
            curr_term: Term(0),
            server_list,
            voted_for: None,
            log: Log::new(),
            timer: Timer::new(clock),
        }
    }

    // # Compliance: Fig 2
    // commitIndex: index of highest log entry known to be committed (initialized to 0, increases
    // monotonically)
    pub fn last_committed_term_idx(&self) -> TermIdx {
        self.log.last_committed_term_idx()
    }
}
