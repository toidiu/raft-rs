use crate::io::{RxReady, TxReady, IO_BUF_LEN};
use core::task::{Context, Poll, Waker};
use std::{
    collections::VecDeque,
    io::Read,
    ops::Deref,
    sync::{Arc, Mutex},
};

pub trait NetRx {
    fn recv(&mut self, data: Vec<u8>);

    fn poll_rx_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to a Future to check for new messages
    fn rx_ready(&mut self) -> RxReady<Self> {
        RxReady(self)
    }
}

pub trait NetTx {
    fn send(&mut self) -> Option<Vec<u8>>;

    fn poll_tx_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to a Future to check for new messages
    fn tx_ready(&mut self) -> TxReady<Self> {
        TxReady(self)
    }
}

/// A handle to the underlying BufferIo
#[derive(Debug, Clone)]
pub struct NetworkIo {
    pub rx: Arc<Mutex<VecDeque<u8>>>,
    pub tx: Arc<Mutex<VecDeque<u8>>>,
    pub rx_waker: Arc<Mutex<Option<Waker>>>,
    pub tx_waker: Arc<Mutex<Option<Waker>>>,
}

impl NetRx for NetworkIo {
    fn recv(&mut self, data: Vec<u8>) {
        println!("  network <--- {:?}", data);

        self.rx.lock().unwrap().extend(data);
        if let Some(waker) = self.rx_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }

    fn poll_rx_ready(&mut self, cx: &mut Context) -> Poll<()> {
        let rdy = !self.rx.lock().unwrap().is_empty();
        *self.rx_waker.lock().unwrap() = Some(cx.waker().clone());
        if rdy {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl NetTx for NetworkIo {
    fn send(&mut self) -> Option<Vec<u8>> {
        let mut buf = [0; IO_BUF_LEN];
        let len = self.tx.lock().unwrap().read(&mut buf[0..]).ok()?;
        if len > 0 {
            println!("  ---> network {:?}", &buf[0..len]);
            Some(buf[0..len].to_vec())
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
