use core::time::Duration;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Clock {
    // The instant when the Clock starts.
    start: Instant,

    // The time now since start.
    now: Instant,
}

impl Clock {
    /// Returns an armed Clock.
    pub fn new() -> Self {
        let start = Instant::now();

        Clock { start, now: start }
    }

    /// Time elapsed since start of the process.
    pub(crate) fn elapsed(&self) -> Duration {
        self.start - Instant::now()
    }

    /// Returns the current Instant.
    pub fn now(&self) -> Instant {
        self.now
    }

    /// Update the Clock's now time.
    pub fn update_now(&mut self, now_instant: Instant) {
        assert!(now_instant >= self.now);

        self.now = now_instant;
    }
}
