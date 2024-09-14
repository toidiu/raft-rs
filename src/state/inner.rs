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

    // # Compliance: Fig 2
    // votedFor: candidateId that received vote in current
    voted_for: Option<ServerId>,

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
    pub fn new(clock: Clock) -> Self {
        Inner {
            curr_term: Term(0),
            voted_for: None,
            log: Log::new(),
            timer: Timer::new(clock),
        }
    }

    // # Compliance: Fig 2
    // commitIndex: index of highest log entry known to be committed (initialized to 0, increases
    // monotonically)
    fn commit_idx(&self) -> Option<TermIdx> {
        self.log.last_committed_term_idx()
    }
}
