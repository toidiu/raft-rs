#![allow(unused)]

fn main() {
    println!("Hello, world!");
}

enum State {
    Follower,
    Leader,
    Candidate,
}

// A term is a logical clock
struct Term(usize);

struct CandidateId(usize);

struct LogEntry {
    term: Term,
    command: Command,
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

    // ==== volatile state on leaders
    // for each server, idx of next log entry to send to that server
    next_idx: usize,
    // for each server, idx of highest log entry known to be replicated on server
    match_idx: usize,
}

enum Command {
    // Leader election
    RequestVote,

    // Add entries and heartbeat
    AppendEntries,
}
