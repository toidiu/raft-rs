use crate::io::{Rx, Tx};
use core::task::{Context, Poll, Waker};
use std::{
    collections::VecDeque,
    io::Read,
    ops::Deref,
    sync::{Arc, Mutex},
};

/// A handle to the underlying BufferIo
#[derive(Debug)]
pub struct ServerIo {
    pub rx: Arc<Mutex<VecDeque<u8>>>,
    pub tx: Arc<Mutex<VecDeque<u8>>>,
    pub waker: Arc<Mutex<Option<Waker>>>,
}

impl Rx for ServerIo {
    fn recv(&mut self) -> Option<Vec<u8>> {
        let mut buf = [0; 100];
        let len = self.rx.lock().unwrap().read(&mut buf[0..]).ok()?;
        if len > 0 {
            Some(buf[0..len].to_vec())
        } else {
            None
        }
    }

    fn poll_rx_ready(&mut self, cx: &mut Context) -> Poll<()> {
        let rdy = !self.rx.lock().unwrap().is_empty();
        *self.waker.lock().unwrap() = Some(cx.waker().clone());
        if rdy {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl Tx for ServerIo {
    fn send(&mut self, data: Vec<u8>) {
        if let Some(waker) = self.waker.lock().unwrap().deref() {
            println!("1-----------32 WAKE");
            waker.wake_by_ref();
        }
        self.tx.lock().unwrap().extend(data);
    }

    fn poll_tx_ready(&mut self, cx: &mut Context) -> Poll<()> {
        let rdy = !self.tx.lock().unwrap().is_empty();
        *self.waker.lock().unwrap() = Some(cx.waker().clone());
        if rdy {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
