use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use rand::Rng;
use tokio::time::{sleep_until, Instant, Sleep};

// # Compliance: 5.2
// To prevent split votes in the first place, election timeouts are
// chosen randomly from a fixed interval (e.g., 150–300ms).
const MIN_DURATION: Duration = Duration::from_millis(150);
const MAX_DURATION: Duration = Duration::from_millis(300);

/// A monotonically increasing clock value for the process
#[derive(Debug, Clone, Copy)]
pub struct Clock(Instant);

impl Default for Clock {
    fn default() -> Self {
        Clock(Instant::now())
    }
}

impl Clock {
    fn current_instance(&self) -> Instant {
        self.0 + self.elapsed()
    }

    pub fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
}

/// A timer can be used to set timeouts
#[derive(Debug)]
pub struct Timer {
    // Reference to the server clock
    clock: Clock,

    // The sleep future
    sleep: Pin<Box<Sleep>>,
}

impl Clone for Timer {
    fn clone(&self) -> Self {
        Timer::new(self.clock)
    }
}

impl Timer {
    pub fn new(clock: Clock) -> Self {
        let duration = rand::thread_rng().gen_range(MIN_DURATION..MAX_DURATION);
        let expire = clock.current_instance() + duration;
        let sleep = Box::pin(sleep_until(expire));
        Timer { clock, sleep }
    }

    pub fn poll_ready(&mut self, ctx: &mut Context) -> Poll<()> {
        let poll = self.sleep.as_mut().poll(ctx);

        if poll.is_ready() {
            // rearm the timer so we continue to make progress
            self.rearm();
        }

        poll
    }

    fn rearm(&mut self) {
        let duration = rand::thread_rng().gen_range(MIN_DURATION..MAX_DURATION);
        let expire = self.clock.current_instance() + duration;
        let sleep = Box::pin(sleep_until(expire));

        self.sleep = sleep;
    }
}

impl Future for Timer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let poll = self.as_mut().poll_ready(ctx);
        poll
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_test::task::new_count_waker;
    use tokio::time::sleep;

    #[tokio::test]
    async fn manual_check_elapsed_time() {
        let clock = Clock::default();
        let mut timer = Timer::new(clock);

        let (waker, cnt) = new_count_waker();
        let mut ctx = Context::from_waker(&waker);
        assert!(timer.poll_ready(&mut ctx).is_pending());
        assert_eq!(cnt, 0);

        // wait less than timer target
        sleep(MIN_DURATION).await;
        assert!(timer.poll_ready(&mut ctx).is_pending());
        assert_eq!(cnt, 0);

        sleep(MAX_DURATION).await;
        assert!(timer.poll_ready(&mut ctx).is_ready());
        assert_eq!(cnt, 1);
    }
}
