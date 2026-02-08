use crate::{
    io::{RxReady, IO_BUF_LEN},
    packet::Packet,
};
use core::task::{Context, Poll, Waker};
use s2n_codec::{DecoderBuffer, DecoderValue};
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
    #[cfg(test)]
    fn recv_raw(&mut self) -> Option<Vec<u8>>;

    /// Read bytes and return the wrapper [RecvPacket] which can be used to yield individual
    /// [Packet]'s.
    fn recv_packet(&mut self) -> Option<RecvPacket<'_>>;

    /// Check if there are bytes available in the Ingress queue for the server to process.
    fn poll_ingress_queue_ready(&mut self, cx: &mut Context) -> Poll<()>;

    /// A Future which can be polled to check for new messages in the queue
    fn ingress_queue_ready(&mut self) -> RxReady<'_, Self> {
        RxReady(self)
    }
}

impl ServerIngress for ServerIngressImpl {
    // Retrieve data for the Server to process
    #[cfg(test)]
    fn recv_raw(&mut self) -> Option<Vec<u8>> {
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

    fn recv_packet(&mut self) -> Option<RecvPacket<'_>> {
        let len = self
            .ingress_queue
            .lock()
            .unwrap()
            .read(&mut self.buf)
            .ok()?;

        Some(RecvPacket::new(&self.buf[0..len]))
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

/// A wrapper type around raw bytes that can yield [Packet].
///
/// Implements Iterator which can yield packets.
///
/// TODO
/// The remaining bytes should be handled and merged with the next read call.
pub struct RecvPacket<'a> {
    buf: DecoderBuffer<'a>,
}

impl<'a> RecvPacket<'a> {
    fn new(buf: &'a [u8]) -> Self {
        let buf = DecoderBuffer::new(buf);
        RecvPacket { buf }
    }
}

impl<'a> Iterator for RecvPacket<'a> {
    type Item = Packet;

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.buf.len();
        if len > 0 {
            let (rpc, buf) = Packet::decode(self.buf).expect("should only receive valid RPC bytes");
            // update the buffer to point to the next set of bytes
            self.buf = buf;

            Some(rpc)
        } else {
            None
        }
    }
}

impl Drop for RecvPacket<'_> {
    fn drop(&mut self) {
        if !self.buf.is_empty() {
            panic!("there are unprocessed bytes in RecvPacket. This can happen with partial reads and needs to be handled.")
        }
    }
}
