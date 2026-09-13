use crate::queue::{BoundedQueue, TxReady, IO_BUF_LEN};
use core::task::{Context, Poll, Waker};
use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

/// A handle held by the Network task for sending and receiving bytes.
#[derive(Debug, Clone)]
pub struct NetworkQueueImpl {
    pub buf: [u8; IO_BUF_LEN],
    pub ingress_queue: Arc<Mutex<BoundedQueue>>,
    pub egress_queue: Arc<Mutex<BoundedQueue>>,
    pub ingress_waker: Arc<Mutex<Option<Waker>>>,
    pub egress_waker: Arc<Mutex<Option<Waker>>>,
}

/// Functionality to queue bytes that were received over a socket onto the `ingress_queue`.
pub trait NetIngress {
    /// Push data to the `ingress_queue`.
    fn push_recv_bytes(&mut self, data: Vec<u8>);
}

/// Functionality to de-queue bytes from the `egress_queue` so that they can be sent over the
/// network.
pub trait NetEgress {
    /// Returns data from the `egress_queue` that should to be sent over the network.
    fn get_send(&mut self) -> Option<Vec<u8>>;

    /// MARKME this needs to be called.
    ///
    /// Check if there are bytes available in the egress queue for that can be sent on the network.
    fn poll_egress_queue_ready(&mut self, cx: &mut Context) -> Poll<()>;

    /// A Future which can be polled to check for new messages in the queue
    fn tx_ready(&mut self) -> TxReady<'_, Self> {
        TxReady(self)
    }
}

impl NetIngress for NetworkQueueImpl {
    /// Push data to the `ingress_queue`.
    fn push_recv_bytes(&mut self, data: Vec<u8>) {
        // dbg!("  network <--- {}", &data);

        // If there is not enough space in the queue then the packet is dropped and Raft should
        // retry it later.
        if !self.ingress_queue.lock().unwrap().try_extend(&data) {
            return;
        }

        if let Some(waker) = self.ingress_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }
}

impl NetEgress for NetworkQueueImpl {
    /// Returns data from the `egress_queue` that should to be sent over the network.
    fn get_send(&mut self) -> Option<Vec<u8>> {
        let bytes_to_send = self
            .egress_queue
            .lock()
            .unwrap()
            .read_into(&mut self.buf[0..]);

        if bytes_to_send > 0 {
            // dbg!("  ---> network {}", &self.buf[0..bytes_to_send]);
            Some(self.buf[0..bytes_to_send].to_vec())
        } else {
            None
        }
    }

    fn poll_egress_queue_ready(&mut self, cx: &mut Context) -> Poll<()> {
        // register the shared Waker
        *self.egress_waker.lock().unwrap() = Some(cx.waker().clone());

        let bytes_available = !self.egress_queue.lock().unwrap().is_empty();
        if bytes_available {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
