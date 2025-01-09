use crate::{peer::Peer, server::ServerId};
use std::collections::BTreeMap;

impl Peer {
    pub fn mock(fill: u8) -> (ServerId, Self) {
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