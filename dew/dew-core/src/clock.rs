use core::time::Duration;
use pin_project_lite::pin_project;
use rand::{Rng, RngCore};
use rand_pcg::Pcg32;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};

//% Compliance
//% Election timeout is chosen randomly between 150-300ms
const MIN_ELECTION_REARM_DURATION: u64 = 150;
const MAX_ELECTION_REARM_DURATION: u64 = 300;

//% Compliance:
//% Upon election: send initial empty AppendEntries RPCs (heartbeat) to each server; repeat during
//% idle periods to prevent election timeouts (§5.2)
//
// A Leader re-arms on an interval smaller than the election interval instead of the election
// range. The assumes a max 100ms network delay.
const HEARTBEAT_REARM_DURATION: u64 = 50;

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

struct Heartbeat {
    /// Randomly generate a timout range.
    prng: Pcg32,

    /// Next deadline instant when the Heartbeat fires.
    deadline: Instant,

    /// The current Server mode is used to determine the timeout interval.
    current_mode: CurrentMode,
}

impl Heartbeat {
    fn new(mut prng: Pcg32) -> Self {
        let current_mode = CurrentMode::FollowerCandidate;

        let duration = Self::rearm_duration(&current_mode, &mut prng);
        let deadline = Instant::now() + duration;

        Self {
            prng,
            deadline,
            // Follower is the default starting mode for a Raft server.
            current_mode,
        }
    }

    /// Returns a Future which can be polled to check if the Heartbeat has expired.
    pub(crate) fn heartbeat_ready(&mut self) -> HeartbeatReady<'_> {
        HeartbeatReady { heartbeat: self }
    }

    // /// Check if the timeout has expired.
    // fn poll_ready(&mut self, ctx: &mut Context) -> Poll<()> {
    //     let mut sleep = self.sleep.lock().unwrap();
    //     sleep.as_mut().poll(ctx)
    // }

    /// Reset and set a new deadline.
    ///
    /// Sets the next timeout to a duration relative to `Instant::now()`.
    pub fn reset_timeout(&mut self) {
        let duration = Self::rearm_duration(&self.current_mode, &mut self.prng);
        let new_deadline = Instant::now() + duration;

        self.deadline = new_deadline;
    }

    /// Randomly select a duration for the next timeout.
    fn rearm_duration<R: RngCore>(mode: &CurrentMode, prng: &mut R) -> Duration {
        let range = match mode {
            CurrentMode::FollowerCandidate => {
                prng.gen_range(MIN_ELECTION_REARM_DURATION..=MAX_ELECTION_REARM_DURATION)
            }
            CurrentMode::Leader => HEARTBEAT_REARM_DURATION,
        };
        Duration::from_millis(range)
    }

    pub fn on_leader(&mut self) {
        self.current_mode = CurrentMode::Leader;
    }

    pub fn on_follower_candidate(&mut self) {
        self.current_mode = CurrentMode::FollowerCandidate;
    }

    // /// The Instant this Timeout next expires. This is needed to support a discrete event
    // /// simulator.
    // #[cfg(any(test, feature = "testing"))]
    // pub fn deadline(&self) -> Instant {
    //     self.deadline
    // }
}

// The current Server mode.
#[derive(Debug, Clone)]
enum CurrentMode {
    FollowerCandidate,
    Leader,
}

trait Timeout {
    /// Duration this timeout will expire in.
    fn deadline(&self) -> Duration;

    /// Has the timeout has expired.
    fn has_expired(&self) -> bool;
}

impl Timeout for Heartbeat {
    fn deadline(&self) -> Duration {
        let now = Instant::now();

        if now >= self.deadline {
            Duration::ZERO
        } else {
            now - self.deadline
        }
    }

    fn has_expired(&self) -> bool {
        if self.deadline() == Duration::ZERO {
            true
        } else {
            false
        }
    }
}

pin_project! {
    /// A handle to check if the Heartbeat has expired.
    pub(crate) struct HeartbeatReady<'a> {
        #[pin]
        heartbeat: &'a mut Heartbeat,
    }
}

impl Future for HeartbeatReady<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if this.heartbeat.has_expired() {
            this.heartbeat.reset_timeout();
            return Poll::Ready(());
        } else {
            Poll::Pending
        }
    }
    //
    //     //     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    //     //         let mut this = self.project();
    //     //         let poll = this.timeout.as_mut().poll_ready(cx);
    //     //
    //     //         // rearm the timeout if expired to ensure perpetual progress
    //     //         if poll.is_ready() {
    //     //             this.timeout.reset_timeout();
    //     //         }
    //     //         poll
    //     //     }
}
