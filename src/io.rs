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
        //   ServerIo                    NetworkIo
        //    Server   <- pop [__rx__]  <- push  NetworkInterface/Socket
        //            push -> [__tx__]  -> pop
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

/// A handle to the underlying BufferIo
#[derive(Debug)]
pub struct ServerIo {
    rx: Arc<Mutex<VecDeque<u8>>>,
    tx: Arc<Mutex<VecDeque<u8>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

/// A handle to the underlying BufferIo
#[derive(Debug, Clone)]
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

        network_io.push(vec![1]);
        network_io.push(vec![2]);
        server_io.push(vec![3]);
        server_io.push(vec![4]);

        assert_eq!(server_io.pop(), Some(vec![1, 2]));
        assert_eq!(server_io.pop(), None);

        assert_eq!(network_io.pop(), Some(vec![3, 4]));
        assert_eq!(network_io.pop(), None);

        network_io.push(vec![5]);
        server_io.push(vec![6]);
        assert_eq!(server_io.pop(), Some(vec![5]));
        assert_eq!(server_io.pop(), None);

        assert_eq!(network_io.pop(), Some(vec![6]));
        assert_eq!(network_io.pop(), None);
    }
}
