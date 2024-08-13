use crate::{
    clock::Clock,
    state::{Term, Timer},
};

#[derive(Debug, Default)]
pub struct InnerState {
    pub common: Common,
}

impl InnerState {
    pub fn new(clock: Clock) -> Self {
        let common = Common::new(clock);
        InnerState { common }
    }
}

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
