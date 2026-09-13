//! Knobs the simulation runs on.

use std::time::Duration;

// Raft has no guaranteed liveness bound. A run of split votes can go on arbitrarily long, so cap
// the event count and let a cluster that never converges fail an assertion instead of hanging.
pub(super) const MAX_EVENTS: usize = 10_000;

// The resolution of the simulated clock. Deadlines closer together than this fire as one event,
// which is what makes a simultaneous split vote reachable at all.
pub(super) const CLOCK_EPSILON: Duration = Duration::from_millis(1);
