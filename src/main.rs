#![allow(unused)]

use core::time::Duration;

fn main() {
    let mut server = Server::new(CandidateId(1));

    loop {
        server.poll();
    }
}

// monotonically increasing clock
struct Clock(usize);

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
    next_idx: Vec<(CandidateId, usize)>,
    // for each server, idx of highest log entry known to be replicated on server
    match_idx: Vec<(CandidateId, usize)>,

    heartbeat_send_timeout: Clock,
}

struct Candidate {
    heartbeat_recv_timeout: Clock,
}

// A term is a logical clock
struct Term(usize);

struct CandidateId(usize);

struct LogEntry {
    term: Term,
    command: Command,
}

enum Command {
    // Leader election
    RequestVote,

    // Add entries and heartbeat
    AppendEntries,
}

struct Server {
    id: CandidateId,
    state: State,

    // ==== persistent state
    current_term: Term,
    voted_for: Option<CandidateId>,
    log: Vec<LogEntry>,

    // ==== volatile state
    // idx of highest log entry known to be committed
    commit_idx: usize,
    // idx of the highest log entry applied to the state machine
    last_applied: usize,
}

impl Server {
    pub fn new(id: CandidateId) -> Self {
        todo!()
    }

    fn poll(&mut self) {
        match self.state {
            State::Follower => todo!(),
            State::Leader => todo!(),
            State::Candidate => todo!(),
        }
    }

    fn on_timeout(&mut self) {
        match self.state {
            State::Follower => todo!(),
            State::Leader => todo!(),
            State::Candidate => todo!(),
        }
    }
}
