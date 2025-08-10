use crate::{
    io::{RxReady, IO_BUF_LEN},
    rpc::Rpc,
};
use core::task::{Context, Poll, Waker};
use s2n_codec::{DecoderBuffer, DecoderValue, EncoderBuffer, EncoderValue};
use std::{
    collections::VecDeque,
    io::Read,
    ops::Deref,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub struct ServerIoIngress {
    pub rx_buf: [u8; IO_BUF_LEN],
    pub ingress_queue: Arc<Mutex<VecDeque<u8>>>,
    pub rx_waker: Arc<Mutex<Option<Waker>>>,
}

#[derive(Debug)]
pub struct ServerIoEgress {
    pub tx_buf: [u8; IO_BUF_LEN],
    pub egress_queue: Arc<Mutex<VecDeque<u8>>>,
    pub tx_waker: Arc<Mutex<Option<Waker>>>,
}

impl ServerRx for ServerIoIngress {
    #[cfg(test)]
    fn recv_raw(&mut self) -> Option<Vec<u8>> {
        let len = self
            .ingress_queue
            .lock()
            .unwrap()
            .read(&mut self.rx_buf)
            .ok()?;
        if len > 0 {
            Some(self.rx_buf[0..len].to_vec())
        } else {
            None
        }
    }

    fn recv_rpc(&mut self) -> Option<RecvRpc> {
        let len = self
            .ingress_queue
            .lock()
            .unwrap()
            .read(&mut self.rx_buf)
            .ok()?;

        Some(RecvRpc::new(&self.rx_buf[0..len]))
    }

    fn poll_rx_ready(&mut self, cx: &mut Context) -> Poll<()> {
        let rdy = !self.ingress_queue.lock().unwrap().is_empty();
        *self.rx_waker.lock().unwrap() = Some(cx.waker().clone());
        if rdy {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl ServerTx for ServerIoEgress {
    #[cfg(test)]
    fn send_raw(&mut self, data: &[u8]) {
        self.egress_queue.lock().unwrap().extend(data.iter());

        if let Some(waker) = self.tx_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }

    fn send_rpc(&mut self, mut rpc: Rpc) {
        let mut buf = EncoderBuffer::new(&mut self.tx_buf);
        rpc.encode_mut(&mut buf);
        let data = buf.as_mut_slice();

        self.egress_queue.lock().unwrap().extend(data.iter());

        if let Some(waker) = self.tx_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }
}

pub trait ServerRx {
    #[cfg(test)]
    fn recv_raw(&mut self) -> Option<Vec<u8>>;

    fn recv_rpc(&mut self) -> Option<RecvRpc>;

    fn poll_rx_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to a Future to check for new messages
    fn rx_ready(&mut self) -> RxReady<Self> {
        RxReady(self)
    }
}

pub trait ServerTx {
    #[cfg(test)]
    fn send_raw(&mut self, _data: &[u8]) {
        unimplemented!()
    }

    fn send_rpc(&mut self, rpc: Rpc);
}

pub struct RecvRpc<'a> {
    // buf: &'a [u8],
    buf: DecoderBuffer<'a>,
}

impl<'a> RecvRpc<'a> {
    fn new(buf: &'a [u8]) -> Self {
        let buf = DecoderBuffer::new(buf);
        RecvRpc { buf }
    }
}

impl<'a> Iterator for RecvRpc<'a> {
    type Item = Rpc;

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.buf.len();
        if len > 0 {
            let (rpc, buf) = Rpc::decode(self.buf).expect("should only receive valid RPC bytes");
            // update the buffer to point to the next set of bytes
            self.buf = buf;

            Some(rpc)
        } else {
            None
        }
    }
}
