use crate::{clock::Clock, io, log, rpc::Rpc};

pub struct Server<IO: io::IO<Rpc>> {
    id: CandidateId,
    state: State,

    // ==== persistent state
    current_term: log::Term,
    voted_for: Option<CandidateId>,
    log: log::Log,

    // ==== volatile state
    // idx of highest log entry known to be committed
    commit_idx: log::Idx,
    // idx of the highest log entry applied to the state machine
    last_applied: log::TermIdx,

    io: IO,
}

impl<IO: io::IO<Rpc>> Server<IO> {}

#[derive(Default)]
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

pub struct CandidateId(u64);
