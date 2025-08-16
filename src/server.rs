use crate::{mode::Mode, raft_state::RaftState};
use std::collections::BTreeMap;

mod id;
mod peer;

pub use id::ServerId;
pub use peer::PeerInfo;

struct Server {
    mode: Mode,
    state: RaftState,
    peer_map: BTreeMap<ServerId, PeerInfo>,
}
