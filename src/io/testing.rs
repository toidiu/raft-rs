use crate::{
    io::{ServerEgress, ServerIngress, IO_BUF_LEN},
    rpc::Rpc,
};
use s2n_codec::{DecoderBuffer, EncoderBuffer, EncoderValue};
use std::{collections::VecDeque, task::Poll};

#[derive(Debug)]
pub struct MockIo {
    pub send_queue: VecDeque<Vec<u8>>,
    pub recv_queue: VecDeque<Vec<u8>>,
}

impl MockIo {
    pub fn new() -> Self {
        MockIo {
            send_queue: VecDeque::new(),
            recv_queue: VecDeque::new(),
        }
    }
}

impl ServerEgress for MockIo {
    #[cfg(test)]
    fn send_raw(&mut self, data: &[u8]) {
        self.send_queue.push_back(data.to_vec());
    }

    fn send_rpc(&mut self, mut rpc: Rpc) {
        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        rpc.encode_mut(&mut buf);
        let data = buf.as_mut_slice();

        self.send_raw(data)
    }
}

impl ServerIngress for MockIo {
    #[cfg(test)]
    fn recv_raw(&mut self) -> Option<Vec<u8>> {
        self.recv_queue.pop_front()
    }

    fn poll_ingress_queue_ready(&mut self, _cx: &mut std::task::Context) -> Poll<()> {
        if self.recv_queue.is_empty() {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

pub fn helper_inspect_sent_rpc(peer_io: &mut MockIo) -> Rpc {
    let rpc_bytes = peer_io.send_queue.pop_front().unwrap();
    assert!(peer_io.send_queue.is_empty());
    let buffer = DecoderBuffer::new(&rpc_bytes);
    let (sent_rpc, _) = buffer.decode::<Rpc>().unwrap();
    sent_rpc
}
