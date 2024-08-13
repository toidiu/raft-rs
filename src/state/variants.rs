use crate::{clock::Clock, state::common::Common};
use uuid::Uuid;

#[derive(Debug)]
pub struct Follower {
    pub common: Common,
}

impl Follower {
    pub fn new(clock: Clock) -> Self {
        let common = Common::new(clock);
        Follower { common }
    }
}

#[derive(Debug)]
pub struct Leader {
    pub common: Common,
    // // ==== volatile state on leaders
    // // for each server, idx of next log entry to send to that server
    // next_idx: Vec<(ServerId, u64)>,
    // // for each server, idx of highest log entry known to be replicated on server
    // match_idx: Vec<(ServerId, u64)>,

    // heartbeat_send_timeout: Clock,
}

#[derive(Debug)]
pub struct Candidate {
    pub common: Common,
}

impl Candidate {
    pub fn new(common: Common) -> Self {
        Candidate { common }
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
