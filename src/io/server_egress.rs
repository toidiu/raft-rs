use crate::{
    io::IO_BUF_LEN,
    rpc::{Header, Packet, Rpc},
    server::ServerId,
};
use core::task::Waker;
use s2n_codec::{EncoderBuffer, EncoderValue};
use std::{
    collections::VecDeque,
    ops::Deref,
    sync::{Arc, Mutex},
};

/// A handle held by the Raft server task.
#[derive(Debug)]
pub struct ServerEgressImpl {
    pub server_id: ServerId,
    pub buf: [u8; IO_BUF_LEN],
    pub egress_queue: Arc<Mutex<VecDeque<u8>>>,
    pub egress_waker: Arc<Mutex<Option<Waker>>>,
}

pub trait ServerEgress {
    #[cfg(test)]
    // Push data to the egress_queue
    fn send_raw(&mut self, data: &[u8]);

    fn send_rpc(&mut self, rpc: Rpc, to: ServerId);
}

impl ServerEgress for ServerEgressImpl {
    #[cfg(test)]
    fn send_raw(&mut self, data: &[u8]) {
        dbg!("  server ---> {:?}", &data);

        self.egress_queue.lock().unwrap().extend(data);

        if let Some(waker) = self.egress_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }

    fn send_rpc(&mut self, rpc: Rpc, to: ServerId) {
        let mut buf = EncoderBuffer::new(&mut self.buf);

        let header = Header {
            from: self.server_id,
            to,
        };
        let packet = Packet::new(header, rpc);
        packet.encode(&mut buf);

        let data = buf.as_mut_slice();
        self.egress_queue.lock().unwrap().extend(data.iter());

        if let Some(waker) = self.egress_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }
}
