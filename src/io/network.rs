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
pub struct NetworkIO {
    pub buf: [u8; IO_BUF_LEN],
    pub rx_queue: Arc<Mutex<VecDeque<u8>>>,
    pub tx_queue: Arc<Mutex<VecDeque<u8>>>,
    pub rx_waker: Arc<Mutex<Option<Waker>>>,
    pub tx_waker: Arc<Mutex<Option<Waker>>>,
}

pub trait NetRx {
    // Push data to the rx_queue
    fn recv(&mut self, data: Vec<u8>);
}

pub trait NetTx {
    // Send data over the network
    fn send(&mut self) -> Option<Vec<u8>>;

    fn poll_tx_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A Future which can be polled to check for new messages in the queue
    fn tx_ready(&mut self) -> TxReady<Self> {
        TxReady(self)
    }
}

impl NetRx for NetworkIO {
    fn recv(&mut self, data: Vec<u8>) {
        dbg!("  network <--- {:?}", &data);

        self.rx_queue.lock().unwrap().extend(data);
        if let Some(waker) = self.rx_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }
}

impl NetTx for NetworkIO {
    fn send(&mut self) -> Option<Vec<u8>> {
        let bytes_to_send = self
            .tx_queue
            .lock()
            .unwrap()
            .read(&mut self.buf[0..])
            .ok()?;
        if bytes_to_send > 0 {
            dbg!("  ---> network {:?}", &self.buf[0..bytes_to_send]);
            Some(self.buf[0..bytes_to_send].to_vec())
        } else {
            None
        }
    }

    fn poll_tx_ready(&mut self, cx: &mut Context) -> Poll<()> {
        // register the shared Waker
        *self.tx_waker.lock().unwrap() = Some(cx.waker().clone());

        let bytes_available = !self.tx_queue.lock().unwrap().is_empty();
        if bytes_available {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
