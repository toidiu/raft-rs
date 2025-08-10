use crate::io::{TxReady, IO_BUF_LEN};
use core::task::{Context, Poll, Waker};
use std::{
    collections::VecDeque,
    io::Read,
    ops::Deref,
    sync::{Arc, Mutex},
};

pub trait NetRx {
    fn recv_from_socket(&mut self, data: Vec<u8>);
}

pub trait NetTx {
    fn send_to_socket(&mut self) -> Option<Vec<u8>>;

    fn poll_tx_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to a Future to check for new messages
    fn tx_ready(&mut self) -> TxReady<Self> {
        TxReady(self)
    }
}

/// A handle to the underlying BufferIo
#[derive(Debug, Clone)]
pub struct NetworkIo {
    pub buf: [u8; IO_BUF_LEN],
    pub rx: Arc<Mutex<VecDeque<u8>>>,
    pub tx: Arc<Mutex<VecDeque<u8>>>,
    pub rx_waker: Arc<Mutex<Option<Waker>>>,
    pub tx_waker: Arc<Mutex<Option<Waker>>>,
}

impl NetRx for NetworkIo {
    fn recv_from_socket(&mut self, data: Vec<u8>) {
        println!("  network <--- {:?}", data);

        self.rx.lock().unwrap().extend(data);
        if let Some(waker) = self.rx_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }
}

impl NetTx for NetworkIo {
    fn send_to_socket(&mut self) -> Option<Vec<u8>> {
        let len = self.tx.lock().unwrap().read(&mut self.buf[0..]).ok()?;
        if len > 0 {
            println!("  ---> network {:?}", &self.buf[0..len]);
            Some(self.buf[0..len].to_vec())
        } else {
            None
        }
    }

    fn poll_tx_ready(&mut self, cx: &mut Context) -> Poll<()> {
        let rdy = !self.tx.lock().unwrap().is_empty();
        *self.tx_waker.lock().unwrap() = Some(cx.waker().clone());
        if rdy {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
