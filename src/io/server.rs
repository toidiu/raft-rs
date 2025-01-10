use crate::io::{RxReady, IO_BUF_LEN};
use core::task::{Context, Poll, Waker};
use std::{
    collections::VecDeque,
    io::Read,
    ops::Deref,
    sync::{Arc, Mutex},
};

/// A handle held by the Raft server task.
#[derive(Debug)]
pub struct ServerIoImpl {
    pub buf: [u8; IO_BUF_LEN],
    pub rx_queue: Arc<Mutex<VecDeque<u8>>>,
    pub tx_queue: Arc<Mutex<VecDeque<u8>>>,
    pub rx_waker: Arc<Mutex<Option<Waker>>>,
    pub tx_waker: Arc<Mutex<Option<Waker>>>,
}

pub trait ServerRx {
    fn recv(&mut self) -> Option<Vec<u8>>;

    fn poll_rx_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A Future which can be polled to check for new messages in the queue
    fn rx_ready(&mut self) -> RxReady<Self> {
        RxReady(self)
    }
}

pub trait ServerTx {
    // Push data to the tx_queue
    fn send(&mut self, data: Vec<u8>);
}

impl ServerRx for ServerIoImpl {
    // Retrieve data for the Server to process
    fn recv(&mut self) -> Option<Vec<u8>> {
        let bytes_to_recv = self
            .rx_queue
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

    fn poll_rx_ready(&mut self, cx: &mut Context) -> Poll<()> {
        // register the shared Waker
        *self.rx_waker.lock().unwrap() = Some(cx.waker().clone());

        let bytes_available = !self.rx_queue.lock().unwrap().is_empty();
        if bytes_available {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl ServerTx for ServerIoImpl {
    fn send(&mut self, data: Vec<u8>) {
        dbg!("  server ---> {:?}", &data);

        self.tx_queue.lock().unwrap().extend(data);

        if let Some(waker) = self.tx_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }
}
