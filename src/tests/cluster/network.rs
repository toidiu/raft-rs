//! The packet router: one server's egress becomes another's ingress.

use crate::{
    packet::Packet,
    queue::{NetEgress, NetIngress},
    server::Id,
    tests::cluster::Cluster,
};
use s2n_codec::{DecoderBuffer, DecoderValue};

/// A packet that has left its sender and not yet reached its destination.
///
/// This is the wire. Keeping it as cluster state rather than a local makes "sent" and "arrived"
/// two separate moments, and it is where a dropped, delayed, reordered, or duplicated packet would
/// be introduced.
pub struct InFlightPacket {
    // The ServerId to route the packet to.
    to: Id,

    // The bytes in the packet.
    bytes: Vec<u8>,
}

/// Did the network make any progress (deliver or process packets).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct NetworkOutcome {
    /// Packets handed to a running server's ingress.
    pub delivered_packets: usize,

    /// Packets addressed to a crashed server, dropped on the floor.
    pub dropped_packets: usize,

    /// Packets still on the wire when the pass ended.
    pub in_flight_packets: usize,
}

impl NetworkOutcome {
    /// Packets were delivered/processed or placed on the wire.
    pub fn did_process_packets(&self) -> bool {
        self.delivered_packets > 0 || self.in_flight_packets > 0
    }
}

impl Cluster {
    /// Move the network forward one pass: deliver what is in flight, let the servers act on it,
    /// then put what they sent onto the wire.
    ///
    /// Collecting last is what keeps a packet from being delivered in the same pass that sent it.
    /// A reply lands in `in_flight_packets` at the end of this pass and is not handed to its
    /// destination until the next one, so a test can observe it in between.
    ///
    /// TODO: inject packet drop or delay into the system.
    pub(super) fn process_packets(&mut self) -> NetworkOutcome {
        let mut outcome = self.dispatch_packet_to_server();
        self.process_ingress_packets();
        self.dispatch_packets_to_network();

        outcome.in_flight_packets = self.in_flight_packets.len();
        outcome
    }

    /// Hand every in-flight packet to its destination's ingress.
    fn dispatch_packet_to_server(&mut self) -> NetworkOutcome {
        let mut outcome = NetworkOutcome::default();

        for packet in std::mem::take(&mut self.in_flight_packets) {
            let Some(node) = self.nodes.iter_mut().find(|node| node.id() == packet.to) else {
                panic!("packet addressed to an unknown server");
            };

            if node.has_crashed() {
                // Silently dropped. Raft makes no distinction between an unreachable peer and a
                // lost packet, so there is nothing to report back to the sender.
                outcome.dropped_packets += 1;
                continue;
            }

            node.queue.push_recv_bytes(packet.bytes);
            outcome.delivered_packets += 1;
        }

        outcome
    }

    /// Let every running server read its ingress and act on what arrived.
    fn process_ingress_packets(&mut self) {
        for node in self.healthy_nodes_mut() {
            node.server.recv();
        }
    }

    /// Drain every running server's egress onto the wire.
    fn dispatch_packets_to_network(&mut self) {
        let mut sent = Vec::new();

        for node in self.healthy_nodes_mut() {
            // The queue is a byte stream, not a message queue: a broadcast to N peers arrives here
            // as one run of concatenated packets, and may span more than one read.
            while let Some(bytes) = node.queue.get_send() {
                let mut buf = DecoderBuffer::new(&bytes);

                // Since a broadcast's packets are addressed to different servers we need to split
                // the bytes being sent into one packet per destination.
                //
                // There is no length prefix on the wire, so finding where a packet ends means
                // decoding its Rpc.
                while !buf.is_empty() {
                    let start = bytes.len() - buf.len();
                    let (packet, remaining) =
                        Packet::decode(buf).expect("server should send valid Packets");
                    buf = remaining;
                    let end = bytes.len() - buf.len();

                    // Forward the sender's exact bytes.
                    //
                    // TODO: inject corrupted data into the system.
                    sent.push(InFlightPacket {
                        to: packet.to(),
                        bytes: bytes[start..end].to_vec(),
                    });
                }
            }
        }

        self.in_flight_packets.extend(sent);
    }
}
