use crate::state::{Clock, Term, Timer};

#[derive(Debug, Default)]
pub(super) struct Common {
    pub curr_term: Term,
    pub timer: Timer,
}

impl Common {
    pub fn new(clock: Clock) -> Self {
        Common {
            curr_term: Term(0),
            timer: Timer::new(clock),
        }
    }
}
