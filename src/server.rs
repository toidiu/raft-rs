use crate::{clock::Clock, io, log};

#[derive(Debug)]
pub struct Server<T: io::Io> {
    id: CandidateId,
    state: State,

    // ==== persistent state
    current_term: log::Term,
    voted_for: Option<CandidateId>,
    log: log::Log,

    // ==== volatile state
    /// ## Compliance:
    /// An entry is considered committed if it is safe for that entry to be applied to state machines.
    // idx of highest log entry known to be committed
    commit_idx: u64,
    // idx of the highest log entry applied to the state machine
    last_applied: u64,

    // IO handle to send and receive Rpc messages
    io_producer: T,
}

impl<T: io::Io> Server<T> {
    fn new(producer: T) -> Server<T> {
        Server {
            id: Default::default(),
            state: Default::default(),
            current_term: Default::default(),
            voted_for: Default::default(),
            log: Default::default(),
            commit_idx: Default::default(),
            last_applied: Default::default(),
            io_producer: producer,
        }
    }
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Follower,
    Leader,
    Candidate,
}

struct Follower {
    heartbeat_recv_timeout: Clock,
}

struct Leader {
    // ==== volatile state on leaders
    // for each server, idx of next log entry to send to that server
    next_idx: Vec<(CandidateId, u64)>,
    // for each server, idx of highest log entry known to be replicated on server
    match_idx: Vec<(CandidateId, u64)>,

    heartbeat_send_timeout: Clock,
}

struct Candidate {
    heartbeat_recv_timeout: Clock,
}

#[derive(Debug, Default)]
pub struct CandidateId(u64);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::BufferIo;

    #[test]
    fn create_server() {
        let (_c, p) = BufferIo::default().split();
        let _server = Server::new(p);
    }
}
