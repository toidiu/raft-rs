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
pub(crate) use server::{ServerIoEgress, ServerIoIngress, ServerRx, ServerTx};

// The size of the buffer used to send/recv from the IO queues
pub const IO_BUF_LEN: usize = 1024;

/// A VecDeque backed IO buffer
pub struct BufferIo;

impl BufferIo {
    pub fn split_server_and_network() -> (ServerIoIngress, ServerIoEgress, NetworkIo) {
        // ```
        //
        //                      [ ingress-queue ]
        //             <- recv                      <- recv_from_socket
        //
        //    [ServerIo]                                          [NetworkIo]
        //
        //             send ->                      -> send_to_socket
        //                      [ egress-queue ]
        // ```
        let ingress_queue = Arc::new(Mutex::new(VecDeque::with_capacity(1024)));
        let egress_queue = Arc::new(Mutex::new(VecDeque::with_capacity(1024)));
        let rx_waker = Arc::new(Mutex::new(None));
        let tx_waker = Arc::new(Mutex::new(None));

        let network_io = NetworkIo {
            buf: [0; IO_BUF_LEN],
            ingress_queue: ingress_queue.clone(),
            egress_queue: egress_queue.clone(),
            rx_waker: rx_waker.clone(),
            tx_waker: tx_waker.clone(),
        };

        let server_io_ingress = ServerIoIngress {
            rx_buf: [0; IO_BUF_LEN],
            ingress_queue: ingress_queue.clone(),
            rx_waker: rx_waker.clone(),
        };

        let server_io_egress = ServerIoEgress {
            tx_buf: [0; IO_BUF_LEN],
            egress_queue: egress_queue.clone(),
            tx_waker: tx_waker.clone(),
        };

        // let server_io = ServerIo {
        //     rx_buf: [0; IO_BUF_LEN],
        //     tx_buf: [0; IO_BUF_LEN],
        //     ingress_queue,
        //     egress_queue,
        //     rx_waker: rx_waker.clone(),
        //     tx_waker: tx_waker.clone(),
        // };
        (server_io_ingress, server_io_egress, network_io)
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

impl_io_ready!(ServerIoIngress, RxReady, poll_rx_ready);
impl_io_ready!(NetworkIo, TxReady, poll_tx_ready);

#[cfg(test)]
mod tests {
    use super::*;

    // network::recv and server::send are separate queues
    #[test]
    fn producer_consumer() {
        let (mut server_io_ingress, mut server_io_egress, mut network_io) =
            BufferIo::split_server_and_network();

        // send and receive multiple times
        for _ in 0..10 {
            // push to network queue
            network_io.recv_from_socket(vec![1, 2]);
            network_io.recv_from_socket(vec![3, 4]);
            // push to server queue
            server_io_egress.send_raw(&[5, 6]);

            // recv from network queue
            assert_eq!(server_io_ingress.recv_raw(), Some(vec![1, 2, 3, 4]));
            assert_eq!(server_io_ingress.recv_raw(), None);
            // recv from server queue
            assert_eq!(network_io.send_to_socket(), Some(vec![5, 6]));
            assert_eq!(network_io.send_to_socket(), None);
        }
    }
}
