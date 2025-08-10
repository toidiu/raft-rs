use crate::{
    io::{ServerEgress, ServerIngress},
    rpc::Rpc,
};
use s2n_codec::DecoderBuffer;
use std::task::Poll;

#[derive(Debug)]
pub struct MockIo {
    pub send_queue: Vec<Vec<u8>>,
    pub recv_queue: Vec<Vec<u8>>,
}

impl MockIo {
    pub fn new() -> Self {
        MockIo {
            send_queue: vec![],
            recv_queue: vec![],
        }
    }
}

impl ServerEgress for MockIo {
    fn send(&mut self, data: Vec<u8>) {
        self.send_queue.push(data);
    }
}

impl ServerIngress for MockIo {
    fn recv(&mut self) -> Option<Vec<u8>> {
        self.recv_queue.pop()
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
    let rpc_bytes = peer_io.send_queue.pop().unwrap();
    assert!(peer_io.send_queue.is_empty());
    let buffer = DecoderBuffer::new(&rpc_bytes);
    let (sent_rpc, _) = buffer.decode::<Rpc>().unwrap();
    sent_rpc
}
