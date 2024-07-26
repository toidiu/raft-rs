use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::time::{sleep_until, Instant, Sleep};

// # Compliance: 5.2
// To prevent split votes in the first place, election timeouts are
// chosen randomly from a fixed interval (e.g., 150–300ms).
const MIN_REARM_DURATION: Duration = Duration::from_millis(150);
const MAX_REARM_DURATION: Duration = Duration::from_millis(300);
const TEST_REARM_DURATION: Duration = Duration::from_millis(200);

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

/// Used for awaiting timeouts.
///
/// To ensure Raft continues to make progress, the timer
/// is automatically rearmed after it expires.
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
        let duration = Self::rearm_duration();
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

    pub fn rearm(&mut self) {
        let duration = Self::rearm_duration();
        let expire = self.clock.current_instance() + duration;
        self.sleep.as_mut().reset(expire);
    }

    pub fn expire(&self) -> Instant {
        self.sleep.deadline()
    }

    fn rearm_duration() -> Duration {
        cfg_if::cfg_if! {
                if #[cfg(test)] {
                    TEST_REARM_DURATION
                } else {
                    use rand::Rng;
                    rand::thread_rng().gen_range(MIN_REARM_DURATION..MAX_REARM_DURATION)
                }
        }
    }
}

impl Future for Timer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        self.as_mut().poll_ready(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_test::task::new_count_waker;
    use tokio::time::advance;

    #[tokio::test]
    async fn manual_check_elapsed_time() {
        tokio::time::pause();
        let mut timer = Timer::new(Clock::default());

        let (waker, cnt) = new_count_waker();
        let mut ctx = Context::from_waker(&waker);
        assert!(timer.poll_ready(&mut ctx).is_pending());
        assert_eq!(cnt, 0);

        // wait less than timer target
        advance(TEST_REARM_DURATION).await;
        assert!(timer.poll_ready(&mut ctx).is_pending());
        assert_eq!(cnt, 0);

        advance(Duration::from_millis(1)).await;
        assert!(timer.poll_ready(&mut ctx).is_ready());
        assert_eq!(cnt, 1);
    }

    #[tokio::test]
    async fn rearm() {
        tokio::time::pause();
        let mut timer = Timer::new(Clock::default());
        let original_expire = timer.expire();

        let (waker, cnt) = new_count_waker();
        let mut ctx = Context::from_waker(&waker);
        assert!(timer.poll_ready(&mut ctx).is_pending());
        assert_eq!(cnt, 0);

        // wait till timer is expired and call poll again
        let rearm_time_plus_1 = TEST_REARM_DURATION + Duration::from_millis(1);
        advance(rearm_time_plus_1).await;
        assert!(timer.poll_ready(&mut ctx).is_ready());
        assert_eq!(cnt, 1);

        // timer should have rearmed
        assert_eq!(original_expire + rearm_time_plus_1, timer.expire());
    }
}
