use crate::{
    io::{ServerRx, ServerTx},
    rpc::Rpc,
};
use s2n_codec::DecoderBuffer;
use std::task::Poll;

#[derive(Debug)]
pub struct MockIO {
    pub send_queue: Vec<Vec<u8>>,
    pub recv_queue: Vec<Vec<u8>>,
}

impl MockIO {
    pub fn new() -> Self {
        MockIO {
            send_queue: vec![],
            recv_queue: vec![],
        }
    }
}

impl ServerTx for MockIO {
    fn send(&mut self, data: Vec<u8>) {
        self.send_queue.push(data);
    }
}

impl ServerRx for MockIO {
    fn recv(&mut self) -> Option<Vec<u8>> {
        self.recv_queue.pop()
    }

    fn poll_rx_ready(&mut self, _cx: &mut std::task::Context) -> Poll<()> {
        if self.recv_queue.is_empty() {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

pub fn helper_inspect_sent_rpc(peer_io: &mut MockIO) -> Rpc {
    let rpc_bytes = peer_io.send_queue.pop().unwrap();
    assert!(peer_io.send_queue.is_empty());
    let buffer = DecoderBuffer::new(&rpc_bytes);
    let (sent_rpc, _) = buffer.decode::<Rpc>().unwrap();
    sent_rpc
}
