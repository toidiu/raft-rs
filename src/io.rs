use core::task::Waker;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

mod rx;
#[cfg(test)]
pub(crate) mod testing;
mod tx;

pub use rx::Rx;
pub use tx::Tx;

/// A VecDeque backed IO buffer
pub struct BufferIo;

impl BufferIo {
    pub fn split() -> (ServerIo, NetworkIo) {
        //   Producer                    Consumer
        //    Server  <- recv [__rx__]  <- send  NetworkInterface/Socket
        //            send -> [__tx__]  -> recv
        let rx = Arc::new(Mutex::new(VecDeque::with_capacity(1024)));
        let tx = Arc::new(Mutex::new(VecDeque::with_capacity(1024)));
        let waker = Arc::new(Mutex::new(None));

        let network_io = NetworkIo {
            rx: tx.clone(),
            tx: rx.clone(),
            waker: waker.clone(),
        };
        let server_io = ServerIo { rx, tx, waker };
        (server_io, network_io)
    }
}

/// A handle to the underlying BufferIO
#[derive(Debug)]
pub struct ServerIo {
    rx: Arc<Mutex<VecDeque<u8>>>,
    tx: Arc<Mutex<VecDeque<u8>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

/// A handle to the underlying BufferIO
#[derive(Debug)]
pub struct NetworkIo {
    rx: Arc<Mutex<VecDeque<u8>>>,
    tx: Arc<Mutex<VecDeque<u8>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producer_consumer() {
        let (mut server_io, mut network_io) = BufferIo::split();

        network_io.send(vec![1]);
        network_io.send(vec![2]);
        server_io.send(vec![3]);
        server_io.send(vec![4]);

        assert_eq!(server_io.recv(), Some(vec![1, 2]));
        assert_eq!(server_io.recv(), None);

        assert_eq!(network_io.recv(), Some(vec![3, 4]));
        assert_eq!(network_io.recv(), None);

        network_io.send(vec![5]);
        server_io.send(vec![6]);
        assert_eq!(server_io.recv(), Some(vec![5]));
        assert_eq!(server_io.recv(), None);

        assert_eq!(network_io.recv(), Some(vec![6]));
        assert_eq!(network_io.recv(), None);
    }
}
