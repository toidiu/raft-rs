use crate::{
    io::{ServerEgress, ServerIngress, IO_BUF_LEN},
    packet::{Packet, Rpc},
    server::{PeerId, ServerId},
};
use s2n_codec::{DecoderBuffer, EncoderBuffer, EncoderValue};
use std::{collections::VecDeque, task::Poll};

#[derive(Debug)]
pub struct MockIo {
    server_id: ServerId,
    pub send_queue: VecDeque<Vec<u8>>,
    pub recv_queue: VecDeque<Vec<u8>>,
}

impl MockIo {
    pub fn new(server_id: ServerId) -> Self {
        MockIo {
            server_id,
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

    fn send_packet(&mut self, to: PeerId, rpc: Rpc) {
        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);

        let packet = Packet::new_send(self.server_id, to, rpc);
        packet.encode(&mut buf);
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

    fn recv_packet(&mut self) -> Option<super::server_ingress::RecvPacket<'_>> {
        unimplemented!()
    }
}

pub fn helper_inspect_one_sent_packet(peer_io: &mut MockIo) -> Packet {
    let packet = helper_inspect_next_sent_packet(peer_io);
    assert!(peer_io.send_queue.is_empty());
    packet
}

pub fn helper_inspect_next_sent_packet(peer_io: &mut MockIo) -> Packet {
    let rpc_bytes = peer_io.send_queue.pop_front().unwrap();
    // assert!(peer_io.send_queue.is_empty());

    let buffer = DecoderBuffer::new(&rpc_bytes);
    let (packet, _) = buffer.decode::<Packet>().unwrap();

    packet
}
