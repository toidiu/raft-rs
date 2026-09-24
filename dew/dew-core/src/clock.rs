use core::time::Duration;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Clock {
    // The instant when the Clock starts.
    start: Instant,
}

impl Clock {
    /// Returns an armed Clock.
    pub fn new() -> Self {
        let start = Instant::now();

        // let duration = Self::rearm_duration(&current_mode, &mut prng);
        // let expire = Instant::now() + duration;
        // let sleep = Box::pin(sleep_until(expire));
        // let sleep = Arc::new(Mutex::new(sleep));

        Clock { start }
    }

    /// Time elapsed since start of the process.
    pub(crate) fn elapsed(&self) -> Duration {
        self.start - Instant::now()
    }
}
