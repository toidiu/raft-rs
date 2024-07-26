use core::task::Poll;
use core::time::Duration;
use tokio::time::Instant;

// A monotonically increasing value
pub struct Clock(Instant);

impl Clock {
    pub fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }

    pub fn current_time(&self) -> Instant {
        self.0 + self.elapsed()
    }
}

struct Timer {
    // reference to the server clock
    clock: Clock,
    // The Instant at which the timer should expire
    target: Option<Instant>,
}

impl Timer {
    fn is_ready(&self) -> bool {
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
