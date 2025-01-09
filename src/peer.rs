use crate::{
    io::{BufferIo, ServerIO, ServerIoImpl, ServerRx, ServerTx},
    server::ServerId,
};

#[cfg(test)]
mod testing;

#[derive(Debug)]
pub struct Peer<T: ServerIO> {
    pub id: ServerId,
    pub io: T,
}

impl<T: ServerIO> Peer<T> {
    pub fn new(id: ServerId, io: T) -> Self {
        Peer { id, io }
    }

    pub fn send(&mut self, data: Vec<u8>) {
        self.io.send(data);
    }

    pub fn recv(&mut self) -> Option<Vec<u8>> {
        self.io.recv()
    }
}

impl<T: ServerIO> PartialOrd for Peer<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl<T: ServerIO> Ord for Peer<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl<T: ServerIO> PartialEq for Peer<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: ServerIO> Eq for Peer<T> {}
