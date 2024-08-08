use core::{pin::Pin, task::Context, time::Duration};
use rand::Rng;
use std::{future::Future, task::Poll};
use tokio::time::{sleep_until, Instant, Sleep};

// # Compliance: 5.2
// To prevent split votes in the first place, election timeouts are
// chosen randomly from a fixed interval (e.g., 150–300ms).
const MIN_DURATION: Duration = Duration::from_millis(150);
const MAX_DURATION: Duration = Duration::from_millis(300);

/// A monotonically increasing clock value for the process
#[derive(Debug)]
pub struct Clock(Instant);

impl Default for Clock {
    fn default() -> Self {
        Clock(Instant::now())
    }
}

impl Clock {
    fn current_time(&self) -> Instant {
        self.0 + self.elapsed()
    }

    fn elapsed(&self) -> Duration {
        let elapsed = self.0.elapsed();
        println!("elapsed: {:?}", elapsed);
        elapsed
    }
}

/// A timer can be used to set timeouts
#[derive(Debug)]
pub struct Timer {
    // Reference to the server clock
    clock: Clock,

    // The Instant at which the timer should expire
    expire_target: Option<Instant>,

    // The sleep future
    sleep: Pin<Box<Sleep>>,
}

impl Timer {
    pub fn new(clock: Clock) -> Self {
        let duration = rand::thread_rng().gen_range(MIN_DURATION..MAX_DURATION);

        let expire = clock.current_time() + duration;
        let sleep = Box::pin(sleep_until(expire));
        Timer {
            clock,
            expire_target: Some(expire),
            sleep,
        }
    }

    fn poll_ready(&mut self, ctx: &mut Context) -> Poll<()> {
        dbg!("---{}", self.clock.elapsed());

        // Only poll the inner timer if we have a target set
        if self.expire_target.is_none() {
            return Poll::Pending;
        }

        let poll = self.sleep.as_mut().poll(ctx);

        if poll.is_ready() {
            self.expire_target = None;
        }

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
