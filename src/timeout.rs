use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use pin_project_lite::pin_project;
use rand::{Rng, RngCore};
use tokio::time::{sleep_until, Instant, Sleep};

// # Compliance: 5.2
// To prevent split votes in the first place, election timeouts are
// chosen randomly from a fixed interval (e.g., 150–300ms).
const MIN_REARM_DURATION: u64 = 150;
const MAX_REARM_DURATION: u64 = 300;

/// A auto-rearming Timeout which can be used to make perpetual progress based on a timeout
/// duration.
///
/// The `timeout_ready() -> TimeoutReady` returns a Future which must be polled to make progress.
/// Upon expiration, TimeoutReady will auto-rearm the Timeout (relative to `Instant::now()`) to
/// ensure perpetual progress.
///
/// Due to auto-rearming, a Timeout is meant to be reused even after it expires:
///
/// ``` none
/// let mut prng = Pcg32::from_entropy();
/// let mut timout = Timeout::new(&mut prng);
///
/// // await the first timeout
/// pin!(timout.timeout_ready(&mut prng)).await;
///
/// // await the second timeout
/// pin!(timout.timeout_ready(&mut prng)).await;
///
/// ```
#[derive(Debug)]
pub struct Timeout {
    // A sleep future which completes after the specified duration.
    //
    // https://docs.rs/tokio/1.40.0/tokio/time/fn.sleep.html
    // > Sleep operates at millisecond granularity and should not be used for tasks that require
    // > high-resolution timers.
    sleep: Pin<Box<Sleep>>,
}

impl Timeout {
    /// Returns an armed Timeout.
    fn new<R: RngCore>(prng: &mut R) -> Self {
        let duration = Self::rearm_duration(prng);
        let expire = Instant::now() + duration;
        let sleep = Box::pin(sleep_until(expire));

        Timeout { sleep }
    }

    /// Check if the timeout has expired.
    fn poll_ready(&mut self, ctx: &mut Context) -> Poll<()> {
        self.sleep.as_mut().poll(ctx)
    }

    /// Reset the expiration time.
    ///
    /// Sets the next timeout to a duration relative to `Instant::now()`.
    fn rearm<R: RngCore>(&mut self, prng: &mut R) {
        let duration = Self::rearm_duration(prng);
        let expire = Instant::now() + duration;

        // reset the sleep future
        self.sleep.as_mut().reset(expire);
    }

    /// Randomly select a duration for the next timeout.
    fn rearm_duration<R: RngCore>(prng: &mut R) -> Duration {
        let range = prng.gen_range(MIN_REARM_DURATION..MAX_REARM_DURATION);
        Duration::from_millis(range)
    }

    /// Returns the instant when the timout will expire.
    fn expire_instant(&self) -> Instant {
        self.sleep.deadline()
    }

    /// Returns a Future which can be polled to check if the timout has expired.
    fn timeout_ready<'a, R: RngCore>(&'a mut self, prng: &'a mut R) -> TimeoutReady<'a, R> {
        TimeoutReady { timout: self, prng }
    }
}

pin_project! {
    /// A handle to check if the timout has expired.
    ///
    /// Auto-rearms the Timout upon expiration.
    struct TimeoutReady<'a, R: RngCore> {
        #[pin]
        timout: &'a mut Timeout,
        prng: &'a mut R
    }
}

impl<'a, R: RngCore> Future for TimeoutReady<'a, R> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let poll = this.timout.as_mut().poll_ready(cx);

        // rearm the timout if expired to ensure perpetual progress
        if poll.is_ready() {
            this.timout.rearm(&mut this.prng);
        }
        poll
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::pin::pin;
    use futures_test::task::new_count_waker;
    use rand::SeedableRng;
    use rand_pcg::Pcg32;
    use tokio::time::advance;

    #[tokio::test]
    async fn manually_check_elapsed_time() {
        tokio::time::pause();
        let mut prng = Pcg32::from_seed([0; 16]);
        let mut timout = Timeout::new(&mut prng);

        let (waker, cnt) = new_count_waker();
        let mut ctx = Context::from_waker(&waker);

        let mut timeout_rdy = pin!(timout.timeout_ready(&mut prng));
        assert!(timeout_rdy.as_mut().poll(&mut ctx).is_pending());
        assert_eq!(cnt, 0);

        // wait less than timout target
        advance(Duration::from_millis(149)).await;
        let mut timeout_rdy = pin!(timout.timeout_ready(&mut prng));
        assert!(timeout_rdy.as_mut().poll(&mut ctx).is_pending());
        assert_eq!(cnt, 0);

        // wait timout duration more so the timout expires
        advance(Duration::from_millis(
            MAX_REARM_DURATION - MIN_REARM_DURATION,
        ))
        .await;
        let mut timeout_rdy = pin!(timout.timeout_ready(&mut prng));
        assert!(timeout_rdy.as_mut().poll(&mut ctx).is_ready());
        assert_eq!(cnt, 1);
    }

    #[tokio::test]
    async fn test_rearm() {
        tokio::time::pause();
        let mut prng = Pcg32::from_seed([0; 16]);
        let mut timout = Timeout::new(&mut prng);
        let original_expire = timout.expire_instant();

        let (waker, cnt) = new_count_waker();
        let mut ctx = Context::from_waker(&waker);
        let max_expiration_time = Duration::from_millis(MAX_REARM_DURATION);

        let mut timeout_rdy = pin!(timout.timeout_ready(&mut prng));
        assert!(timeout_rdy.as_mut().poll(&mut ctx).is_pending());
        assert_eq!(cnt, 0);

        // wait till timout is expired and call poll again
        advance(max_expiration_time).await;
        let mut timeout_rdy = pin!(timout.timeout_ready(&mut prng));
        assert!(timeout_rdy.as_mut().poll(&mut ctx).is_ready());
        assert_eq!(cnt, 1);

        // timout should have rearmed but not expired
        assert!(original_expire + max_expiration_time < timout.expire_instant());
        let mut timeout_rdy = pin!(timout.timeout_ready(&mut prng));
        assert!(timeout_rdy.as_mut().poll(&mut ctx).is_pending());
        assert_eq!(cnt, 1);

        // wait till timout is expired and call poll again
        let max_expiration_time = Duration::from_millis(MAX_REARM_DURATION);
        advance(max_expiration_time).await;
        let mut timeout_rdy = pin!(timout.timeout_ready(&mut prng));
        assert!(timeout_rdy.as_mut().poll(&mut ctx).is_ready());
        assert_eq!(cnt, 2);
    }
}
