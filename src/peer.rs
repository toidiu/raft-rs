use crate::{
    io::{BufferIo, ServerIO, ServerRx, ServerTx},
    server::ServerId,
};

#[derive(Debug)]
pub struct Peer {
    pub id: ServerId,
    pub io: ServerIO,
}

impl Peer {
    pub fn new(id: ServerId) -> Self {
        // FIXME pass in IO when this is used
        let (io, _) = BufferIo::split();

        Peer { id, io }
    }

    pub fn send(&mut self, data: Vec<u8>) {
        self.io.send(data);
    }

    pub fn recv(&mut self) -> Option<Vec<u8>> {
        self.io.recv()
    }
}

impl PartialOrd for Peer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl Ord for Peer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialEq for Peer {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Peer {}
