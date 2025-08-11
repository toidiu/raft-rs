use crate::{io::ServerEgress, rpc::Rpc, server::ServerId};

#[derive(Debug)]
pub struct PeerInfo {
    pub id: ServerId,
}

impl PeerInfo {
    pub fn new(id: ServerId) -> Self {
        PeerInfo { id }
    }

    pub fn send_rpc<E: ServerEgress>(&mut self, io_egress: &mut E, rpc: Rpc) {
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
    use crate::{peer::PeerInfo, server::ServerId};
    use std::collections::BTreeMap;

    impl PeerInfo {
        fn mock(fill: u8) -> (ServerId, Self) {
            let id = ServerId::new([fill; 16]);
            (id, Self::new(id))
        }

        pub fn mock_as_map(ids_fill: &[u8]) -> BTreeMap<ServerId, Self> {
            let mut map = BTreeMap::new();
            for id in ids_fill {
                let (id, peer) = Self::mock(*id);
                map.insert(id, peer);
            }

            map
        }
    }
}
