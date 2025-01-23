use crate::{io::ServerEgress, rpc::Rpc, server::ServerId};

#[derive(Debug, Clone, Copy)]
pub struct PeerInfo {
    pub id: ServerId,
}

impl PeerInfo {
    pub fn new(id: ServerId) -> Self {
        PeerInfo { id }
    }

    pub fn send_rpc<E: ServerEgress>(&self, rpc: Rpc, io_egress: &mut E) {
        // TODO address to this peer
        io_egress.send_rpc(rpc);
    }
}

impl PartialOrd for PeerInfo {
    #[allow(clippy::non_canonical_partial_ord_impl)]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl Ord for PeerInfo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialEq for PeerInfo {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for PeerInfo {}

#[cfg(test)]
mod testing {
    use crate::server::{PeerInfo, ServerId};

    impl PeerInfo {
        pub fn mock_list(peer_id: &[ServerId]) -> Vec<PeerInfo> {
            let mut vec = Vec::new();
            for id in peer_id {
                let peer_info = PeerInfo::new(*id);
                vec.push(peer_info);
            }

            vec
        }
    }
}
