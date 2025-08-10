use crate::{io::ServerEgress, server::ServerId};

#[derive(Debug)]
pub struct Peer<E: ServerEgress> {
    pub id: ServerId,
    pub io_egress: E,
}

impl<E: ServerEgress> Peer<E> {
    pub fn new(id: ServerId, io_egress: E) -> Self {
        Peer { id, io_egress }
    }

    pub fn send(&mut self, data: Vec<u8>) {
        self.io_egress.send(data);
    }
}

impl<E: ServerEgress> PartialOrd for Peer<E> {
    #[allow(clippy::non_canonical_partial_ord_impl)]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl<E: ServerEgress> Ord for Peer<E> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl<E: ServerEgress> PartialEq for Peer<E> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<E: ServerEgress> Eq for Peer<E> {}

#[cfg(test)]
mod testing {
    use crate::{io::testing::MockIo, peer::Peer, server::ServerId};
    use std::collections::BTreeMap;

    impl Peer<MockIo> {
        fn mock(fill: u8) -> (ServerId, Self) {
            let id = ServerId::new([fill; 16]);
            // FIXME pass in IO when this is used

            (id, Self::new(id, MockIo::new()))
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
