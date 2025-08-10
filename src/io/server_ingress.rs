use crate::io::{RxReady, IO_BUF_LEN};
use core::task::{Context, Poll, Waker};
use std::{
    collections::VecDeque,
    io::Read,
    sync::{Arc, Mutex},
};

/// A handle held by the Raft server task.
#[derive(Debug)]
pub struct ServerIngressImpl {
    pub buf: [u8; IO_BUF_LEN],
    pub ingress_queue: Arc<Mutex<VecDeque<u8>>>,
    pub ingress_waker: Arc<Mutex<Option<Waker>>>,
}

pub trait ServerIngress {
    fn recv(&mut self) -> Option<Vec<u8>>;

    fn poll_ingress_queue_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A Future which can be polled to check for new messages in the queue
    fn ingress_queue_ready(&mut self) -> RxReady<Self> {
        RxReady(self)
    }
}

impl ServerIngress for ServerIngressImpl {
    // Retrieve data for the Server to process
    fn recv(&mut self) -> Option<Vec<u8>> {
        let bytes_to_recv = self
            .ingress_queue
            .lock()
            .unwrap()
            .read(&mut self.buf[0..])
            .ok()?;
        if bytes_to_recv > 0 {
            dbg!("  server <--- {:?}", &self.buf[0..bytes_to_recv]);
            Some(self.buf[0..bytes_to_recv].to_vec())
        } else {
            None
        }
    }

    fn poll_ingress_queue_ready(&mut self, cx: &mut Context) -> Poll<()> {
        // register the shared Waker
        *self.ingress_waker.lock().unwrap() = Some(cx.waker().clone());

        let bytes_available = !self.ingress_queue.lock().unwrap().is_empty();
        if bytes_available {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
