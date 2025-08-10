use crate::{
    io::{RxReady, IO_BUF_LEN},
    rpc::Rpc,
};
use core::task::{Context, Poll, Waker};
use s2n_codec::{EncoderBuffer, EncoderValue};
use std::{
    collections::VecDeque,
    io::Read,
    ops::Deref,
    sync::{Arc, Mutex},
};

pub trait ServerRx {
    fn recv(&mut self) -> Option<Vec<u8>>;

    fn poll_rx_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to a Future to check for new messages
    fn rx_ready(&mut self) -> RxReady<Self> {
        RxReady(self)
    }
}

pub trait ServerTx {
    fn send(&mut self, _data: &[u8]) {
        todo!()
    }

    fn send_rpc(&mut self, rpc: Rpc);
}

/// A handle to the underlying BufferIo
#[derive(Debug)]
pub struct ServerIo {
    pub rx_buf: [u8; IO_BUF_LEN],
    pub tx_buf: [u8; IO_BUF_LEN],
    pub rx: Arc<Mutex<VecDeque<u8>>>,
    pub tx: Arc<Mutex<VecDeque<u8>>>,
    pub rx_waker: Arc<Mutex<Option<Waker>>>,
    pub tx_waker: Arc<Mutex<Option<Waker>>>,
}

impl ServerRx for ServerIo {
    fn recv(&mut self) -> Option<Vec<u8>> {
        let len = self.rx.lock().unwrap().read(&mut self.rx_buf).ok()?;
        if len > 0 {
            Some(self.rx_buf[0..len].to_vec())
        } else {
            None
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

impl ServerTx for ServerIo {
    fn send(&mut self, data: &[u8]) {
        println!("  server ---> {:?}", data);

        self.tx.lock().unwrap().extend(data);

        if let Some(waker) = self.tx_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }

    fn send_rpc(&mut self, mut rpc: Rpc) {
        let mut buf = EncoderBuffer::new(&mut self.tx_buf);
        rpc.encode_mut(&mut buf);
        let data = buf.as_mut_slice();

        self.tx.lock().unwrap().extend(data.iter());

        if let Some(waker) = self.tx_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }
}
