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
            buf: [0; IO_BUF_LEN],
            rx: rx_queue.clone(),
            tx: tx_queue.clone(),
            rx_waker: rx_waker.clone(),
            tx_waker: tx_waker.clone(),
        };
        let server_io = ServerIo {
            rx_buf: [0; IO_BUF_LEN],
            tx_buf: [0; IO_BUF_LEN],
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
    use crate::{log::TermIdx, rpc::Rpc};
    use s2n_codec::{EncoderBuffer, EncoderValue};

    // network::recv and server::send are separate queues
    #[test]
    fn producer_consumer() {
        let (mut server_io, mut network_io) = BufferIo::split();

        let mut buf = [0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut buf);
        let (rpc, rpc_data) = {
            let rpc = Rpc::new_append_entry(0, TermIdx::new(0, 1));

            rpc.clone().encode_mut(&mut buf);
            let rpc_data = buf.as_mut_slice();
            (rpc, rpc_data)
        };

        // send and receive multiple times
        for _ in 0..10 {
            // push to network queue
            network_io.recv_from_socket(vec![1, 2]);
            network_io.recv_from_socket(vec![3, 4]);
            // push to server queue
            server_io.send_rpc(rpc);

            // recv from network queue
            assert_eq!(server_io.recv(), Some(vec![1, 2, 3, 4]));
            assert_eq!(server_io.recv(), None);
            // recv from server queue
            assert_eq!(network_io.send_to_socket(), Some(rpc_data.to_vec()));
            assert_eq!(network_io.send_to_socket(), None);
        }
    }
}
