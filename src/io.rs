use core::future::Future;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

mod network;
mod server;
#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
pub(crate) use network::NetRx;
pub(crate) use network::{NetTx, NetworkIo};
pub(crate) use server::{ServerIo, ServerRx, ServerTx};

// The size of the buffer used to send/recv from the IO queues
pub const IO_BUF_LEN: usize = 1024;

/// A VecDeque backed IO buffer
pub struct BufferIo;

impl BufferIo {
    pub fn split() -> (ServerIo, NetworkIo) {
        // ```
        //   [ServerIo]                            [NetworkIo]
        //  server process                       network socket
        //
        //              <- recv [rx-queue]  <- recv
        //              send -> [tx-queue]  -> send
        // ```
        let rx_queue = Arc::new(Mutex::new(VecDeque::with_capacity(1024)));
        let tx_queue = Arc::new(Mutex::new(VecDeque::with_capacity(1024)));
        let rx_waker = Arc::new(Mutex::new(None));
        let tx_waker = Arc::new(Mutex::new(None));

        let network_io = NetworkIo {
            rx: rx_queue.clone(),
            tx: tx_queue.clone(),
            rx_waker: rx_waker.clone(),
            tx_waker: tx_waker.clone(),
        };
        let server_io = ServerIo {
            rx: rx_queue,
            tx: tx_queue,
            rx_waker: rx_waker.clone(),
            tx_waker: tx_waker.clone(),
        };
        (server_io, network_io)
    }
}

// A handle to check the readiness of the rx queue.
//
// While all types have an implicit `Sized` by default, traits are
// `?Sized` by default. The ?Sized marker tells the compiler that
// it is fine for T to be potentially not Sized. Alternatively we
// could also have marked `Rx: Sized`
pub struct RxReady<'a, T: ?Sized>(pub &'a mut T);

// A handle to check the readiness of the tx queue.
//
// While all types have an implicit `Sized` by default, traits are
// `?Sized` by default. The ?Sized marker tells the compiler that
// it is fine for T to be potentially not Sized. Alternatively we
// could also have marked `Tx: Sized`
pub struct TxReady<'a, T: ?Sized>(pub &'a mut T);

macro_rules! impl_io_ready(($io:ident, $fut:ident, $poll_fn:ident) => {
    impl<'a> Future for $fut<'a, $io> {
        type Output = ();

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            self.0.$poll_fn(cx)
        }
    }
});

impl_io_ready!(ServerIo, RxReady, poll_rx_ready);
impl_io_ready!(NetworkIo, TxReady, poll_tx_ready);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producer_consumer() {
        let (mut server_io, mut network_io) = BufferIo::split();

        network_io.recv(vec![1]);
        network_io.recv(vec![2]);
        server_io.send(vec![3]);
        server_io.send(vec![4]);

        assert_eq!(server_io.recv(), Some(vec![1, 2]));
        assert_eq!(server_io.recv(), None);

        assert_eq!(network_io.send(), Some(vec![3, 4]));
        assert_eq!(network_io.send(), None);

        network_io.recv(vec![5]);
        server_io.send(vec![6]);
        assert_eq!(server_io.recv(), Some(vec![5]));
        assert_eq!(server_io.recv(), None);

        assert_eq!(network_io.send(), Some(vec![6]));
        assert_eq!(network_io.send(), None);
    }
}
