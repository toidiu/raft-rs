//! Knobs the simulation runs on.

use std::time::Duration;

// Raft has no guaranteed liveness bound — a run of split votes can go on arbitrarily long. Cap the
// event count so a cluster that never converges reports a failed assertion instead of hanging.
pub(super) const MAX_EVENTS: usize = 10_000;

// The resolution of the simulated clock. Deadlines closer together than this fire as one event,
// which is what makes a simultaneous split vote reachable at all.
pub(super) const CLOCK_EPSILON: Duration = Duration::from_millis(1);
