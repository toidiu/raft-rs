use crate::{
    clock::{Clock, Timer},
    state::Term,
};

#[derive(Debug)]
pub struct Inner {
    pub curr_term: Term,
    pub timer: Timer,
}

impl Inner {
    pub fn new(clock: Clock) -> Self {
        Inner {
            curr_term: Term(0),
            timer: Timer::new(clock),
        }
    }
}
