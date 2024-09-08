use core::{
    future::Future,
    task::{Context, Poll},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

mod network;
mod server;
#[cfg(test)]
pub(crate) mod testing;

pub use network::NetworkIo;
pub use server::ServerIo;

/// A VecDeque backed IO buffer
pub struct BufferIo;

impl BufferIo {
    pub fn split() -> (ServerIo, NetworkIo) {
        //   ServerIo                    NetworkIo
        //    Server  <- recv [__rx__]  <- send  NetworkInterface/Socket
        //            send -> [__tx__]  -> recv
        let rx_queue = Arc::new(Mutex::new(VecDeque::with_capacity(1024)));
        let tx_queue = Arc::new(Mutex::new(VecDeque::with_capacity(1024)));
        let waker = Arc::new(Mutex::new(None));

        let network_io = NetworkIo {
            rx: tx_queue.clone(),
            tx: rx_queue.clone(),
            waker: waker.clone(),
        };
        let server_io = ServerIo {
            rx: rx_queue,
            tx: tx_queue,
            waker,
        };
        (server_io, network_io)
    }
}

pub trait Rx {
    fn recv(&mut self) -> Option<Vec<u8>>;

    fn poll_rx_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to a Future to check for new messages
    fn rx_ready(&mut self) -> RxReady<Self> {
        RxReady(self)
    }
}

pub trait Tx {
    fn send(&mut self, data: Vec<u8>);

    fn poll_tx_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to a Future to check for new messages
    fn tx_ready(&mut self) -> TxReady<Self> {
        TxReady(self)
    }
}

pub trait NetRx {
    fn recv(&mut self, data: Vec<u8>);

    fn poll_rx_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to a Future to check for new messages
    fn rx_ready(&mut self) -> RxReady<Self> {
        RxReady(self)
    }
}

pub trait NetTx {
    fn send(&mut self) -> Option<Vec<u8>>;

    fn poll_tx_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to a Future to check for new messages
    fn tx_ready(&mut self) -> TxReady<Self> {
        TxReady(self)
    }
}

// A handle to check the readiness of the rx queue.
//
// While all types have an implicit `Sized` by default, traits are
// `?Sized` by default. The ?Sized marker tells the compiler that
// it is fine for T to be potentially not Sized. Alternatively we
// could also have marked `Rx: Sized`
pub struct RxReady<'a, T: ?Sized>(pub &'a mut T);

impl<'a, T: Rx> Future for RxReady<'a, T> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.0.poll_rx_ready(cx)
    }
}

// A handle to check the readiness of the tx queue.
//
// While all types have an implicit `Sized` by default, traits are
// `?Sized` by default. The ?Sized marker tells the compiler that
// it is fine for T to be potentially not Sized. Alternatively we
// could also have marked `Tx: Sized`
pub struct TxReady<'a, T: ?Sized>(pub &'a mut T);

impl<'a, T: Tx> Future for TxReady<'a, T> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.0.poll_tx_ready(cx)
    }
}

impl<'a> Future for TxReady<'a, NetworkIo> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.0.poll_tx_ready(cx)
    }
}

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
