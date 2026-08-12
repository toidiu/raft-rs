//! A deterministic, cluster of Servers.
//!
//! This is a discrete-event simulator with two events types:
//!
//! 1. **A packet in flight.** Delivery is instantaneous relative to timeouts, so the network is
//!    always drained to quiescence before the clock moves.
//! 2. **The earliest election timeout.** With the network quiet, jump the clock straight to the
//!    next deadline and let that server fire.

use network::InFlightPacket;
use node::{Node, ServerIdx};
use raft_rs::{
    server::{PeerId, Server, ServerId},
    timeout::Timeout,
};
use rand::SeedableRng;
use rand_pcg::Pcg32;

mod faults;
mod inspect;
mod network;
mod node;
mod sim;
mod variables;

/// Cluster of Server Nodes in this test.
pub struct Cluster {
    nodes: Vec<Node>,

    // Packets that have left a sender and not yet arrived. This is the network itself.
    in_flight_packets: Vec<InFlightPacket>,
}

impl Cluster {
    /// Build `n` servers, each knowing the other `n - 1` as peers, with the clock paused.
    pub fn new(n: usize) -> Cluster {
        // Freeze the clock. From here time only moves when the simulation moves it.
        tokio::time::pause();

        let nodes = {
            // Server Ids for all nodes in this test.
            let server_ids: Vec<ServerId> = (0..n)
                .map(|idx| ServerId::new(Self::unique_bytes(idx)))
                .collect();

            server_ids
                .iter()
                .enumerate()
                .map(|(server_idx, server_id)| {
                    // peer_list holds every other server in the quorum, never self.
                    let peer_list: Vec<PeerId> = server_ids
                        .iter()
                        .enumerate()
                        .filter(|(peer_idx, _)|
                            // Exclude self idx from the list of peers
                            *peer_idx != server_idx)
                        .map(|(_, peer_id)| PeerId::new(*peer_id.as_bytes()))
                        .collect();

                    // A distinct seed per server is what staggers the election timeouts. Give them all
                    // the same seed and every server campaigns on the same tick, forever.
                    let prng = Pcg32::from_seed(Self::unique_bytes(server_idx));

                    let (server, queue) = Server::new(*server_id, peer_list, Timeout::new(prng));

                    Node {
                        server,
                        queue,
                        crashed: false,
                    }
                })
                .collect()
        };

        Cluster {
            nodes,
            in_flight_packets: Vec::new(),
        }
    }

    /// Node by ServerIdx.
    fn node(&self, idx: ServerIdx) -> &Node {
        &self.nodes[idx.0]
    }

    /// Node by ServerIdx.
    fn node_mut(&mut self, idx: ServerIdx) -> &mut Node {
        &mut self.nodes[idx.0]
    }

    /// Healthy nodes in the system.
    fn healthy_nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(|node| !node.has_crashed())
    }

    /// Healthy nodes in the system.
    fn healthy_nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.nodes.iter_mut().filter(|node| !node.has_crashed())
    }

    /// 16 distinct bytes per server position. Servers are addressed by a 16 byte Id and seeded
    /// from 16 bytes.
    fn unique_bytes(idx: usize) -> [u8; 16] {
        let mut bytes = [0; 16];
        // Offset by 1 so no server holds the all-zero pattern, which is the natural "unset" value.
        bytes[..8].copy_from_slice(&(idx as u64 + 1).to_be_bytes());
        bytes
    }
}
