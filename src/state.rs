use crate::{clock::Clock, rpc::Rpc};

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
        State::Follower(Follower::default())
    }
}

impl State {
    pub fn on_timeout(&mut self) {
        todo!()
    }

    pub fn recv_rpc(&mut self, rpc: Rpc) {
        todo!()
    }

    #[cfg(test)]
    fn is_follower(&self) -> bool {
        matches!(self, State::Follower(_))
    }
}

#[derive(Debug, Default)]
pub struct Follower {
    heartbeat_recv_timeout: Clock,
}

#[derive(Debug, Default)]
pub struct Leader {
    // ==== volatile state on leaders
    // for each server, idx of next log entry to send to that server
    next_idx: Vec<(ServerId, u64)>,
    // for each server, idx of highest log entry known to be replicated on server
    match_idx: Vec<(ServerId, u64)>,

    heartbeat_send_timeout: Clock,
}

#[derive(Debug, Default)]
pub struct Candidate {
    heartbeat_recv_timeout: Clock,
}

#[derive(Debug, Default)]
pub struct ServerId(u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let s = State::default();
        assert!(s.is_follower());
    }
}
