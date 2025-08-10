use crate::{io::ServerIO, server::ServerId};

#[derive(Debug)]
pub struct Peer<IO: ServerIO> {
    pub id: ServerId,
    pub io: IO,
}

impl<IO: ServerIO> Peer<IO> {
    pub fn new(id: ServerId, io: IO) -> Self {
        Peer { id, io }
    }

    pub fn send(&mut self, data: Vec<u8>) {
        self.io.send(data);
    }

    pub fn recv(&mut self) -> Option<Vec<u8>> {
        self.io.recv()
    }
}

impl<IO: ServerIO> PartialOrd for Peer<IO> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl<IO: ServerIO> Ord for Peer<IO> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl<IO: ServerIO> PartialEq for Peer<IO> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<IO: ServerIO> Eq for Peer<IO> {}

#[cfg(test)]
mod testing {
    use crate::{io::testing::MockIo, peer::Peer, server::ServerId};
    use std::collections::BTreeMap;

    impl Peer<MockIo> {
        pub fn mock(fill: u8) -> (ServerId, Self) {
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
