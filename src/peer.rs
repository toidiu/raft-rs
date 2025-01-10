use crate::{io::ServerIO, server::ServerId};

#[cfg(test)]
mod testing;

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
