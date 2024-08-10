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

/// Used for awaiting timeouts.
///
/// To ensure Raft continues to make progress, the timer
/// is automatically rearmed after it expires.
#[derive(Debug, Default)]
pub struct Timer {
    // Reference to the server clock
    clock: Clock,

    expire: Option<Instant>,

    // The sleep future
    sleep: Option<Pin<Box<Sleep>>>,
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
        let sleep = Some(Box::pin(sleep_until(expire)));
        Timer {
            clock,
            expire: Some(expire),
            sleep,
        }
    }

    pub fn poll_ready(&mut self, ctx: &mut Context) -> Poll<()> {
        let poll = if let Some(sleep) = &mut self.sleep {
            sleep.as_mut().poll(ctx)
        } else {
            return Poll::Pending;
        };

        if poll.is_ready() {
            // rearm the timer so we continue to make progress
            self.rearm();
        }

        poll
    }

    pub fn rearm(&mut self) {
        let duration = rand::thread_rng().gen_range(MIN_DURATION..MAX_DURATION);
        let expire = self.clock.current_instance() + duration;
        let sleep = Box::pin(sleep_until(expire));

        self.expire = Some(expire);
        self.sleep = Some(sleep);
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
    use tokio::time::sleep;

    #[tokio::test]
    async fn manual_check_elapsed_time() {
        let mut timer = Timer::new(Clock::default());

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

    #[tokio::test]
    async fn rearm() {
        let mut timer = Timer::new(Clock::default());
        assert!(timer.expire.is_some());
        let original_expire = timer.expire.unwrap();

        let (waker, cnt) = new_count_waker();
        let mut ctx = Context::from_waker(&waker);
        assert!(timer.poll_ready(&mut ctx).is_pending());
        assert_eq!(cnt, 0);

        // wait till timer is expired and call poll again
        sleep(MAX_DURATION).await;
        assert!(timer.poll_ready(&mut ctx).is_ready());
        assert_eq!(cnt, 1);

        // timer should have rearmed
        assert!(timer.expire.is_some());
        assert!(original_expire < timer.expire.unwrap());
    }
}
