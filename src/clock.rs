use core::task::Poll;
use core::time::Duration;
use tokio::time::Instant;

// A monotonically increasing value
pub struct Clock(Instant);

impl Default for Clock {
    fn default() -> Self {
        Clock(Instant::now())
    }
}

impl Clock {
    fn elapsed(&self) -> Duration {
        let e = self.0.elapsed();
        println!("{:?}", e);
        e
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
}

impl Timer {
    pub fn new(clock: Clock, target: Duration) -> Self {
        let target = clock.current_time() + target;
        Timer {
            clock,
            target: Some(target),
        }
    }

    pub fn is_ready(&self) -> bool {
        if let Some(target) = self.target {
            let expire_time = target.duration_since(self.clock.current_time());
            if expire_time.is_zero() {
                return true;
            }
        }

        false
    }
}

impl core::future::Future for Timer {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.is_ready() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn timer_ready() {
        let clock = Clock::default();
        let timer = Timer::new(clock, Duration::from_secs(3));
        assert!(!timer.is_ready());

        sleep(Duration::from_secs(2)).await;
        assert!(!timer.is_ready());

        sleep(Duration::from_secs(1)).await;
        assert!(timer.is_ready());
    }
}
