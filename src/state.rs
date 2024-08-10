use crate::{
    clock::{Clock, Timer},
    rpc::{AppendEntries, RequestVote, Rpc},
};
use uuid::Uuid;

/// Raft state diagram.
///
/// 1: startup
/// 2: timeout. start election
/// 3: timeout. new election
/// 4: recv vote from majority of servers
/// 5: discover current leader or new term
/// 6: discover server with higher term
///
///
/// ```none
///     |                       ------
///     | 1                    |  3   |
///     v             2        |      v
/// +----------+ --------->  +-----------+
/// |          |             |           |
/// | Follower |             | Candidate |
/// |          |             |           |
/// +----------+  <--------- +-----------+
///        ^          5             |
///        |                        | 4
///        |                        v
///        |          6        +--------+
///         ------------------ |        |
///                            | Leader |
///                            |        |
///                            +--------+
///
/// ```
/// https://textik.com/#8dbf6540e0dd1676

#[derive(Debug)]
pub enum State {
    Follower(Follower),
    Leader(Leader),
    Candidate(Candidate),
}

impl State {
    pub fn new(clock: Clock) -> Self {
        // 1: startup
        State::Follower(Follower::new(clock))
    }

    pub fn timer(&mut self) -> &mut Timer {
        match self {
            State::Follower(inner) => &mut inner.timer,
            State::Leader(inner) => &mut inner.timer,
            State::Candidate(inner) => &mut inner.timer,
        }
    }

    pub fn on_timeout(&mut self) {
        match self {
            State::Follower(_inner) => {
                // 2: timeout. start election
                self.on_candidate();
            }
            State::Leader(_inner) => {
                self.send_heartbeat();
            }
            State::Candidate(_inner) => {
                // 3: timeout. new election
                self.on_candidate();
            }
        }
    }

    pub fn recv(&mut self, rpc: Rpc) {
        match rpc {
            Rpc::RequestVote(rpc) => self.on_request_vote(rpc),
            Rpc::AppendEntries(rpc) => self.on_append_entry(rpc),
        }
    }

    fn on_candidate(&mut self) {
        println!("state: on_candidate");
        let timer = self.timer().clone();
        *self = State::Candidate(Candidate::new(timer));
        // TODO: start new election
    }

    fn send_heartbeat(&mut self) {
        println!("state: send_heartbeat");
        // TODO send rpc
    }

    fn on_request_vote(&mut self, rpc: RequestVote) {
        println!("state: recv RequestVote. {:?}", rpc.term);
        // TODO: recv vote, request for new election
    }

    fn on_append_entry(&mut self, rpc: AppendEntries) {
        println!("recv AppendEntries. {:?}", rpc.term);
        // TODO: heartbeat, new entry, discover current leader, discover new term
    }
}

#[derive(Debug)]
pub struct Follower {
    timer: Timer,
}

impl Follower {
    fn new(clock: Clock) -> Self {
        Follower {
            timer: Timer::new(clock),
        }
    }
}

#[derive(Debug)]
pub struct Leader {
    timer: Timer,
    // // ==== volatile state on leaders
    // // for each server, idx of next log entry to send to that server
    // next_idx: Vec<(ServerId, u64)>,
    // // for each server, idx of highest log entry known to be replicated on server
    // match_idx: Vec<(ServerId, u64)>,

    // heartbeat_send_timeout: Clock,
}

#[derive(Debug)]
pub struct Candidate {
    timer: Timer,
}

impl Candidate {
    fn new(timer: Timer) -> Self {
        Candidate { timer }
    }
}

#[derive(Debug)]
pub struct ServerId(String);

impl ServerId {
    pub fn new() -> Self {
        let id = Uuid::new_v4();
        ServerId(id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_state() {
        let s = State::new(Clock::default());
        assert!(matches!(s, State::Follower(_)));
    }
}
