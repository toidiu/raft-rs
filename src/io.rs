use core::future::Future;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

mod network;
mod server_egress;
mod server_ingress;

#[cfg(test)]
pub mod testing;

pub use network::{NetTx, NetworkIoImpl};
pub use server_egress::{ServerEgress, ServerEgressImpl};
pub use server_ingress::{ServerIngress, ServerIngressImpl};

pub trait ServerIO: ServerEgress + ServerIngress {}

impl<IO: ServerEgress + ServerIngress> ServerIO for IO {}

// FIXME this is allocated per recv/send. Instead allocate a common buffer that can be reused.
// The default size of the buffer used to send/recv from the IO queues
pub const IO_BUF_LEN: usize = 1024;

/// An [sans-IO](https://sans-io.readthedocs.io) abstraction backed by queues.
///
/// IO is modeled as two queues: ingress-queue and the egress-queue. The ingress-queue represents
/// data received on the network and eventually received by the sever task. The egress-queue
/// represents data sent by the server task and eventually sent over the network. Hence, the
/// NetworkIO handle is responsible for pushing data to the ingress-queue, while the ServerIO
/// handle is responsible is responsible for poping data from the ingress-queue. The egress-queue
/// works in reverse.
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
/// [ ingress ] [ egress ]
/// [ queue   ] [ queue  ]
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
/// To support asynchronous workload, each queue has a Waker associated with it. To indicate
/// readiness, the IO handle pairs have a **shared** reference to a Waker per queue (ingress_waker
/// and egress_waker). To make progress the egress/ingress_ready futures must be polled (i.e.
/// `NetworkIO::egress_ready().poll()` and `ServerIO::ingress_ready().poll()`) to register the
/// Waker with the async runtime.
///
/// Once a waker has been registered, the IO handle pushing new data into the queue can signals
/// readiness for the other IO pair. For example, when the NetworkIO handle receives data on the
/// socket, it pushes data to the ingress-queue and signals its associated Waker. This wakes up the
/// task associated with ServerIO, which can then retrieve and process the data in the
/// ingress-queue.
pub struct BufferIo;

impl BufferIo {
    pub fn split() -> (ServerIngressImpl, ServerEgressImpl, NetworkIoImpl) {
        let ingress_queue = Arc::new(Mutex::new(VecDeque::with_capacity(IO_BUF_LEN)));
        let ingress_waker = Arc::new(Mutex::new(None));

        let egress_queue = Arc::new(Mutex::new(VecDeque::with_capacity(IO_BUF_LEN)));
        let egress_waker = Arc::new(Mutex::new(None));

        let network_io_handle = NetworkIoImpl {
            buf: [0; IO_BUF_LEN],
            ingress_queue: ingress_queue.clone(),
            egress_queue: egress_queue.clone(),
            ingress_waker: ingress_waker.clone(),
            egress_waker: egress_waker.clone(),
        };

        let server_ingress_handle = ServerIngressImpl {
            buf: [0; IO_BUF_LEN],
            ingress_queue,
            ingress_waker,
        };
        let server_egress_handle = ServerEgressImpl {
            buf: [0; IO_BUF_LEN],
            egress_queue,
            egress_waker,
        };

        (
            server_ingress_handle,
            server_egress_handle,
            network_io_handle,
        )
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

// Implement Futures to poll the queue for readiness.
impl_io_ready!(ServerIngressImpl, RxReady, poll_ingress_queue_ready);
impl_io_ready!(NetworkIoImpl, TxReady, poll_egress_queue_ready);

#[cfg(test)]
mod tests {
    use super::*;
    use core::task::Context;
    use futures_test::task::{new_count_waker, AwokenCount};
    use network::NetRx;
    use server_egress::ServerEgress;

    fn test_helper_io_setup() -> (
        ServerIngressImpl,
        ServerEgressImpl,
        NetworkIoImpl,
        AwokenCount,
        AwokenCount,
    ) {
        let (mut server_ingress, server_egress, mut network_io) = BufferIo::split();

        let (ingress_waker, ingress_cnt) = new_count_waker();
        let mut ctx = Context::from_waker(&ingress_waker);
        // A ingress waker must be registed by calling poll_ingress_ready on ServerIO
        let _ = server_ingress.poll_ingress_queue_ready(&mut ctx);
        assert_eq!(ingress_cnt, 0);

        let (egress_waker, egress_cnt) = new_count_waker();
        let mut ctx = Context::from_waker(&egress_waker);
        // A egress waker must be registed by calling poll_egress_ready on NetworkIO
        let _ = network_io.poll_egress_queue_ready(&mut ctx);
        assert_eq!(egress_cnt, 0);

        (
            server_ingress,
            server_egress,
            network_io,
            ingress_cnt,
            egress_cnt,
        )
    }

    #[test]
    fn io_recv() {
        let (mut server_ingress, _server_egress, mut network_io, ingress_cnt, _egress_cnt) =
            test_helper_io_setup();

        // Recv
        network_io.recv(vec![1]);
        assert_eq!(ingress_cnt, 1);
        network_io.recv(vec![2]);
        assert_eq!(ingress_cnt, 2);

        assert_eq!(server_ingress.recv(), Some(vec![1, 2]));
        assert_eq!(server_ingress.recv(), None);
        assert_eq!(ingress_cnt, 2);
    }

    #[test]
    fn io_send() {
        let (_server_ingress, mut server_egress, mut network_io, _ingress_cnt, egress_cnt) =
            test_helper_io_setup();

        // Send
        assert_eq!(egress_cnt, 0);
        server_egress.send(vec![3]);
        assert_eq!(egress_cnt, 1);
        server_egress.send(vec![4]);
        assert_eq!(egress_cnt, 2);

        assert_eq!(network_io.send(), Some(vec![3, 4]));
        assert_eq!(network_io.send(), None);
        assert_eq!(egress_cnt, 2);
    }

    #[test]
    fn io_send_recv() {
        let (mut server_ingress, mut server_egress, mut network_io, ingress_cnt, egress_cnt) =
            test_helper_io_setup();

        // Interleaved send and recv
        network_io.recv(vec![5]);
        assert_eq!(ingress_cnt, 1);
        assert_eq!(egress_cnt, 0);

        server_egress.send(vec![6]);
        assert_eq!(ingress_cnt, 1);
        assert_eq!(egress_cnt, 1);

        assert_eq!(server_ingress.recv(), Some(vec![5]));
        assert_eq!(server_ingress.recv(), None);
        assert_eq!(network_io.send(), Some(vec![6]));
        assert_eq!(network_io.send(), None);

        assert_eq!(ingress_cnt, 1);
        assert_eq!(egress_cnt, 1);
    }
}
