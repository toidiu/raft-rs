#[derive(Debug, Clone, Copy)]
pub enum Rpc {
    RequestVote(RequestVote),
    AppendEntries(AppendEntries),
}

// Leader election
#[derive(Debug, Clone, Copy)]
pub struct RequestVote {}

// Add entries and heartbeat
#[derive(Debug, Clone, Copy)]
pub struct AppendEntries {}
