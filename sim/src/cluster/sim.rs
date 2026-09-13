//! The event loop: what happens next, and when.

use crate::cluster::{
    network::NetworkOutcome,
    node::ServerIdx,
    variables::{CLOCK_EPSILON, MAX_EVENTS},
    Cluster,
};
use std::time::Duration;
use tokio::time::Instant;

/// The outcome of running the next step in the simulation.
#[derive(Debug, PartialEq, Eq)]
pub enum SingleStepOutcome {
    /// In flight packets were routed and every running server processed its inbox.
    Delivered(NetworkOutcome),

    /// The clock jumped to the earliest deadline and fired the timeout.
    FiredTimeout,

    /// The clock reached the caller's time_horizon with no event due before it.
    ReachedMaxTimeHorizon,

    /// No in-flight packets and no running server holds a timer.
    ///
    /// Only happens when every server has been paused since a running Raft cluster always has a
    /// timer armed somewhere.
    Stalled,
}

impl Cluster {
    /// Run until an election has settled, and return the winner.
    ///
    /// Which server wins is not chosen here. It falls out of the timeout race.
    pub async fn elect(&mut self) -> ServerIdx {
        let settled = self
            .run_until_condition(Cluster::election_has_settled)
            .await;

        assert!(settled, "no Leader emerged within the event budget");
        self.leader().expect("just asserted a Leader exists")
    }

    /// Exactly one Leader exists, and every other running server agrees that it leads.
    fn election_has_settled(&self) -> bool {
        let Some(leader_idx) = self.leader() else {
            return false;
        };

        let leader_id = self.as_peer_id(leader_idx);

        // Every running server agrees on the new Leader, the Leader included: it names itself.
        self.idxs()
            .filter(|idx| !self.is_paused(*idx))
            .all(|idx| self.known_leader(idx) == Some(leader_id))
    }

    /// Route packets until the wire is empty, without moving the clock.
    pub fn drain_network(&mut self) {
        for _ in 0..MAX_EVENTS {
            if !self.process_packets().did_process_packets() {
                return;
            }
        }
        panic!("network never went quiet");
    }

    /// Run until `cond` holds. Returns false if the cluster went idle or the budget ran out first.
    pub async fn run_until_condition(&mut self, cond: impl Fn(&Cluster) -> bool) -> bool {
        for _ in 0..MAX_EVENTS {
            if cond(self) {
                return true;
            }
            if self.run_next().await == SingleStepOutcome::Stalled {
                break;
            }
        }
        cond(self)
    }

    /// Run for a span of simulated time.
    pub async fn run_for(&mut self, duration: Duration) {
        let time_horizon = Instant::now() + duration;

        for _ in 0..MAX_EVENTS {
            match self.run_next_until(Some(time_horizon)).await {
                // Reached the end of the time_horizon or equilibrium (Stalled).
                SingleStepOutcome::ReachedMaxTimeHorizon | SingleStepOutcome::Stalled => return,
                SingleStepOutcome::Delivered(_) | SingleStepOutcome::FiredTimeout => (),
            }
        }

        panic!("cluster did not reach the time_horizon within the event budget");
    }

    /// Take the next event, "fast-forwarding" the clock if the network is quiet.
    ///
    /// Whichever comes first:
    /// - deliver in-flight packets
    /// - advance the clock to the earliest timeout and fire it
    pub async fn run_next(&mut self) -> SingleStepOutcome {
        self.run_next_until(None).await
    }

    /// Run the next step in the simulator.
    ///
    /// `time_horizon` caps how far the clock moves.
    async fn run_next_until(&mut self, time_horizon: Option<Instant>) -> SingleStepOutcome {
        // Process, deliver and send packets on the network. All packets are handled/routed before
        // handling timeouts (timeouts are only handled if there are no packets).
        //
        // This means there is no concept of network delay at the moment.
        //
        // TODO: Add packet delay.
        let network_outcome = self.process_packets();

        if network_outcome.did_process_packets() {
            SingleStepOutcome::Delivered(network_outcome)
        } else {
            // The network has nothing left to move, so only the clock can change anything.
            match (self.next_deadline(), time_horizon) {
                // A timeout is due, but not before the caller wants to stop.
                (Some(deadline), Some(time_horizon)) if time_horizon < deadline => {
                    self.advance_time_to(time_horizon).await;
                    SingleStepOutcome::ReachedMaxTimeHorizon
                }
                (None, Some(time_horizon)) => {
                    self.advance_time_to(time_horizon).await;
                    SingleStepOutcome::ReachedMaxTimeHorizon
                }
                (Some(deadline), _) => {
                    // Advance past the deadline so the timeout fires.
                    self.advance_time_to(deadline + CLOCK_EPSILON).await;
                    self.fire_expired_timeouts();
                    SingleStepOutcome::FiredTimeout
                }
                (None, None) => SingleStepOutcome::Stalled,
            }
        }
    }

    /// Advance clock to the provided instant.
    async fn advance_time_to(&self, instant: Instant) {
        let now = Instant::now();

        // An instant already in the past means the event is due now, e.g.
        // - a time_horizon the clock has landed on
        // - a resumed server whose deadline expired while it was down
        if instant > now {
            tokio::time::advance(instant - now).await;
        }
    }

    /// The earliest election timeout across the servers still running.
    fn next_deadline(&self) -> Option<Instant> {
        self.healthy_nodes()
            .map(|node| node.server.timeout_deadline())
            .min()
    }

    /// Fire every timeout that has expired.
    ///
    /// Usually exactly one but two server deadlines can land in the same tick. The random timeout
    /// and quorum are expected to help reach consensus.
    fn fire_expired_timeouts(&mut self) {
        let mut cx = futures_test::task::noop_context();

        for node in self.healthy_nodes_mut() {
            node.server.poll_timeout(&mut cx);
        }
    }
}
