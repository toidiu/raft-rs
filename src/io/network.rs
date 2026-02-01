use crate::io::{TxReady, IO_BUF_LEN};
use core::task::{Context, Poll, Waker};
use std::{
    collections::VecDeque,
    io::Read,
    ops::Deref,
    sync::{Arc, Mutex},
};

/// A handle held by the network task.
#[derive(Debug, Clone)]
pub struct NetworkIoImpl {
    pub buf: [u8; IO_BUF_LEN],
    pub ingress_queue: Arc<Mutex<VecDeque<u8>>>,
    pub egress_queue: Arc<Mutex<VecDeque<u8>>>,
    pub ingress_waker: Arc<Mutex<Option<Waker>>>,
    pub egress_waker: Arc<Mutex<Option<Waker>>>,
}

/// Functionality to queue bytes that were received over a socket onto the `ingress_queue`.
pub trait NetIngress {
    /// Push data to the `ingress_queue`.
    fn recv(&mut self, data: Vec<u8>);
}

/// Functionality to de-queue bytes from the `egress_queue` so that they can be sent over the
/// network.
pub trait NetEgress {
    /// Send data over the network
    fn send(&mut self) -> Option<Vec<u8>>;

    /// Check if there are bytes available in the egress queue for that can be sent on the network.
    fn poll_egress_queue_ready(&mut self, cx: &mut Context) -> Poll<()>;

    /// A Future which can be polled to check for new messages in the queue
    fn tx_ready(&mut self) -> TxReady<'_, Self> {
        TxReady(self)
    }
}

impl NetIngress for NetworkIoImpl {
    fn recv(&mut self, data: Vec<u8>) {
        // dbg!("  network <--- {}", &data);

        self.ingress_queue.lock().unwrap().extend(data);
        if let Some(waker) = self.ingress_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }
}

impl NetEgress for NetworkIoImpl {
    fn send(&mut self) -> Option<Vec<u8>> {
        let bytes_to_send = self
            .egress_queue
            .lock()
            .unwrap()
            .read(&mut self.buf[0..])
            .ok()?;
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
