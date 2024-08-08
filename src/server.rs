use crate::{
    clock::Clock,
    log::{LogEntry, Term},
};

pub struct Server {
    id: CandidateId,
    state: State,

    // ==== persistent state
    current_term: Term,
    voted_for: Option<CandidateId>,
    log: Vec<LogEntry>,

    // ==== volatile state
    // idx of highest log entry known to be committed
    commit_idx: u64,
    // idx of the highest log entry applied to the state machine
    last_applied: u64,
}

impl Server {
    // pub fn new(_id: u64) -> Self {
    //     // get list of peers
    //     // set timer

    //     // initialize
    //     todo!()
    // }

    // pub fn recv(&mut self) {
    //     // check for received commands from the peer

    //     todo!()
    // }

    // pub fn on_timeout(&mut self) {
    //     // check timers

    //     todo!()
    // }
}

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
