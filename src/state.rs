use crate::rpc::Rpc;
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

impl Default for State {
    fn default() -> Self {
        // 1: startup
        State::Follower(Follower::default())
    }
}

impl State {
    pub fn on_timeout(&mut self) {
        match self {
            State::Follower(_) => {
                // 2: timeout. start election
                self.on_candidate();
            }
            State::Leader(_) => {
                self.send_heartbeat();
            }
            State::Candidate(_) => {
                // 3: timeout. new election
                self.on_candidate();
            }
        }
    }

    pub fn recv_rpc(&mut self, rpc: Rpc) {
        match rpc {
            Rpc::RequestVote(_) => self.on_request_vote(),
            Rpc::AppendEntries(_) => self.on_append_entry(),
        }
    }

    fn on_candidate(&mut self) {
        *self = State::Candidate(Candidate::default());
        // TODO: start new election
    }

    fn send_heartbeat(&mut self) {
        *self = State::Candidate(Candidate::default());
        // TODO send rpc
    }

    fn on_request_vote(&mut self) {
        // TODO: recv vote, request for new election
    }

    fn on_append_entry(&mut self) {
        // TODO: heartbeat, new entry, discover current leader, discover new term
    }
}

#[derive(Debug, Default)]
pub struct Follower {}

#[derive(Debug, Default)]
pub struct Leader {
    // // ==== volatile state on leaders
    // // for each server, idx of next log entry to send to that server
    // next_idx: Vec<(ServerId, u64)>,
    // // for each server, idx of highest log entry known to be replicated on server
    // match_idx: Vec<(ServerId, u64)>,

    // heartbeat_send_timeout: Clock,
}

#[derive(Debug, Default)]
pub struct Candidate {}

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

    #[test]
    fn default_state() {
        let s = State::default();
        assert!(matches!(s, State::Follower(_)));
    }
}
