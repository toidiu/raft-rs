use core::future::Future;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::time::Duration;
use tokio::time::sleep_until;
use tokio::time::Instant;
use tokio::time::Sleep;

// A monotonically increasing value
pub struct Clock(Instant);

impl Default for Clock {
    fn default() -> Self {
        Clock(Instant::now())
    }
}

impl Clock {
    fn elapsed(&self) -> Duration {
        let elapsed = self.0.elapsed();
        println!("elapsed: {:?}", elapsed);
        elapsed
    }

    pub fn current_time(&self) -> Instant {
        self.0 + self.elapsed()
    }
}

pub struct Timer {
    // reference to the server clock
    clock: Clock,
    // The Instant at which the timer should expire
    target: Option<Instant>,

    sleep: Pin<Box<Sleep>>,
}

impl Timer {
    pub fn new(clock: Clock, target: Duration) -> Self {
        let target = clock.current_time() + target;
        let sleep = Box::pin(sleep_until(target));
        Timer {
            clock,
            target: Some(target),
            sleep,
        }
    }

    fn is_ready(&mut self, ctx: &mut Context) -> bool {
        dbg!("---{}", self.clock.elapsed());
        let poll = self.sleep.as_mut().poll(ctx);

        if poll.is_ready() {
            self.target = None;
        }

        poll.is_ready()
    }

    pub fn sleep_until(&self) -> Option<Sleep> {
        if let Some(target) = self.target {
            let until = target + self.clock.elapsed();
            dbg!("until {}", until);
            Some(sleep_until(until))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::task::Waker;
    use futures_test::task::new_count_waker;
    use tokio::time::sleep;

    #[tokio::test]
    async fn manual_check_elapsed_time() {
        let clock = Clock::default();
        let mut timer = Timer::new(clock, Duration::from_secs(2));

        let (waker, cnt) = new_count_waker();
        let mut ctx = Context::from_waker(&waker);
        assert!(!timer.is_ready(&mut ctx));
        assert_eq!(cnt, 0);

        // wait less than timer target
        sleep(Duration::from_secs(1)).await;
        assert!(!timer.is_ready(&mut ctx));
        assert_eq!(cnt, 0);

        sleep(Duration::from_secs(1)).await;
        assert!(timer.is_ready(&mut ctx));
        assert_eq!(cnt, 1);
    }

    #[tokio::test]
    async fn sleep_until() {
        let clock = Clock::default();
        let mut timer = Timer::new(clock, Duration::from_secs(2));

        let (waker, cnt) = new_count_waker();
        let mut ctx = Context::from_waker(&waker);
        assert!(!timer.is_ready(&mut ctx));
        assert_eq!(cnt, 0);

        timer.sleep_until().unwrap().await;
        assert!(timer.is_ready(&mut ctx));
        assert_eq!(cnt, 1);
        // confirm that we did infact wait more than target seconds
        assert!(timer.clock.elapsed() > Duration::from_secs(2));
    }
}
