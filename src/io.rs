use core::future::Future;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

mod network;
mod server;

#[cfg(test)]
pub mod testing;

pub use network::{NetTx, NetworkIoImpl};
pub use server::{ServerIoImpl, ServerRx, ServerTx};

pub trait ServerIO: ServerTx + ServerRx {}

impl<IO: ServerTx + ServerRx> ServerIO for IO {}

// FIXME this is allocated per recv/send. Instead allocate a common buffer that can be reused.
// The default size of the buffer used to send/recv from the IO queues
pub const IO_BUF_LEN: usize = 1024;

/// An [sans-IO](https://sans-io.readthedocs.io) abstraction backed by queues.
///
/// IO is modeled as two queues: rx-queue and the tx-queue. The rx-queue represents data
/// received on the network and eventually received by the sever task. The tx-queue
/// represents data sent by the server task and eventually sent over the network. Hence,
/// the NetworkIO handle is responsible for pushing data to the rx-queue, while the
/// ServerIO handle is responsible is responsible for poping data from the rx-queue. The
/// tx-queue works in reverse.
///
/// |-----------------|
/// |  server task    |
/// |-----------------|
/// |-----------------|
/// |  ServerIO       |
/// |  handle         |
/// |                 |
/// |-----------------|
///    ^           v
///    |           |
///    |           |
/// [rx-queue]   [tx-queue]
///    |           |
///    |           |
///    ^           v
/// |-----------------|
/// |                 |
/// |  NetworkIO      |
/// |  handle         |
/// |-----------------|
/// |-----------------|
/// |  network socket |
/// |-----------------|
///
/// To support asynchronous workload, each queue has a Waker associated with it. To
/// indicate readiness, the IO handle pairs have a **shared** reference to a Waker per
/// queue (rx_waker and tx_waker). To make progress the tx/rx_ready futures must be polled
/// (i.e. `NetworkIO::tx_ready().poll()` and `ServerIO::rx_ready().poll()`) to register
/// the Waker with the async runtime.
///
/// Once a waker has been registered, the IO handle pushing new data into the queue can
/// signals readiness for the other IO pair. For example, when the NetworkIO handle
/// receives data on the socket, it pushes data to the rx-queue and signals its associated
/// Waker. This wakes up the task associated with ServerIO, which can then retrieve and
/// process the data in the rx-queue.
pub struct BufferIo;

impl BufferIo {
    pub fn split() -> (ServerIoImpl, NetworkIoImpl) {
        let rx_queue = Arc::new(Mutex::new(VecDeque::with_capacity(IO_BUF_LEN)));
        let rx_waker = Arc::new(Mutex::new(None));

        let tx_queue = Arc::new(Mutex::new(VecDeque::with_capacity(IO_BUF_LEN)));
        let tx_waker = Arc::new(Mutex::new(None));

        let network_io_handle = NetworkIoImpl {
            buf: [0; IO_BUF_LEN],
            rx_queue: rx_queue.clone(),
            tx_queue: tx_queue.clone(),
            rx_waker: rx_waker.clone(),
            tx_waker: tx_waker.clone(),
        };

        let server_io_handle = ServerIoImpl {
            buf: [0; IO_BUF_LEN],
            rx_queue,
            tx_queue,
            rx_waker,
            tx_waker,
        };
        (server_io_handle, network_io_handle)
    }
}

// A handle to check the readiness of the RX queue.
//
// Why the `?Sized` bound?
// While all types have an implicit `Sized` by default, traits are `?Sized` by default.
// The ?Sized marker tells the compiler that it is fine for T to be potentially not Sized.
pub struct RxReady<'a, T: ?Sized>(pub &'a mut T);

// A handle to check the readiness of the TX queue.
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

// Implement Future for Server and Network IO
impl_io_ready!(ServerIoImpl, RxReady, poll_rx_ready);
impl_io_ready!(NetworkIoImpl, TxReady, poll_tx_ready);

#[cfg(test)]
mod tests {
    use super::*;
    use core::task::Context;
    use futures_test::task::{new_count_waker, AwokenCount};
    use network::NetRx;
    use server::ServerTx;

    fn test_helper_io_setup() -> (ServerIoImpl, NetworkIoImpl, AwokenCount, AwokenCount) {
        let (mut server_io, mut network_io) = BufferIo::split();

        let (rx_waker, rx_cnt) = new_count_waker();
        let mut ctx = Context::from_waker(&rx_waker);
        // A rx waker must be registed by calling poll_rx_ready on ServerIO
        let _ = server_io.poll_rx_ready(&mut ctx);
        assert_eq!(rx_cnt, 0);

        let (tx_waker, tx_cnt) = new_count_waker();
        let mut ctx = Context::from_waker(&tx_waker);
        // A tx waker must be registed by calling poll_tx_ready on NetworkIO
        let _ = network_io.poll_tx_ready(&mut ctx);
        assert_eq!(tx_cnt, 0);

        (server_io, network_io, rx_cnt, tx_cnt)
    }

    #[test]
    fn io_recv() {
        let (mut server_io, mut network_io, rx_cnt, _tx_cnt) = test_helper_io_setup();

        // Recv
        network_io.recv(vec![1]);
        assert_eq!(rx_cnt, 1);
        network_io.recv(vec![2]);
        assert_eq!(rx_cnt, 2);

        assert_eq!(server_io.recv(), Some(vec![1, 2]));
        assert_eq!(server_io.recv(), None);
        assert_eq!(rx_cnt, 2);
    }

    #[test]
    fn io_send() {
        let (mut server_io, mut network_io, _rx_cnt, tx_cnt) = test_helper_io_setup();

        // Send
        assert_eq!(tx_cnt, 0);
        server_io.send(vec![3]);
        assert_eq!(tx_cnt, 1);
        server_io.send(vec![4]);
        assert_eq!(tx_cnt, 2);

        assert_eq!(network_io.send(), Some(vec![3, 4]));
        assert_eq!(network_io.send(), None);
        assert_eq!(tx_cnt, 2);
    }

    #[test]
    fn io_send_recv() {
        let (mut server_io, mut network_io, rx_cnt, tx_cnt) = test_helper_io_setup();

        // Interleaved send and recv
        network_io.recv(vec![5]);
        assert_eq!(rx_cnt, 1);
        assert_eq!(tx_cnt, 0);

        server_io.send(vec![6]);
        assert_eq!(rx_cnt, 1);
        assert_eq!(tx_cnt, 1);

        assert_eq!(server_io.recv(), Some(vec![5]));
        assert_eq!(server_io.recv(), None);
        assert_eq!(network_io.send(), Some(vec![6]));
        assert_eq!(network_io.send(), None);

        assert_eq!(rx_cnt, 1);
        assert_eq!(tx_cnt, 1);
    }
}
