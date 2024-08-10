use crate::{
    clock::{Clock, Timer},
    state::Term,
};

#[derive(Debug, Default)]
pub struct Inner {
    pub common: Common,
}

impl Inner {
    pub fn new(clock: Clock) -> Self {
        let common = Common::new(clock);
        Inner { common }
    }
}

#[derive(Debug, Default)]
pub struct Common {
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
