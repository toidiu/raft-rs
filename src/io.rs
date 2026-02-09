use crate::{
    future_util::{EventSelectFuture, EventSelectOutcome},
    io::socket::{SocketRecvReady, SocketSendReady},
    queue::{NetEgress, NetIngress},
    server::{PeerId, Server, ServerId},
    timeout::Timeout,
};
use rand::SeedableRng;
use rand_pcg::Pcg32;
use tokio::net::TcpStream;

mod socket;

const BUF_SIZE: usize = 1024;

async fn start() {
    let prng = Pcg32::from_seed([0; 16]);
    let timeout = Timeout::new(prng);

    let server_id = ServerId::new([1; 16]);
    let peer2_id = PeerId::new([11; 16]);
    let peer3_id = PeerId::new([12; 16]);
    let peer_list = vec![peer2_id, peer3_id];

    let (mut server, mut network_handle) = Server::new(server_id, peer_list.clone(), timeout);

    let stream = TcpStream::connect("127.0.0.1:8080")
        .await
        .expect("TODO: better error handling");

    let mut buf = vec![0; BUF_SIZE];
    loop {
        let (server_recv, server_timeout) = server.select_future();
        let select = EventSelectFuture {
            server_recv,
            server_timeout,
            network_send: network_handle.tx_ready(),
            socket_send: SocketSendReady { stream: &stream },
            socket_recv: SocketRecvReady { stream: &stream },
        };
        let EventSelectOutcome {
            server_recv_rdy,
            server_timeout_rdy,
            network_send_rdy,
            socket_send_rdy,
            socket_recv_rdy,
        } = select.await;

        // Server recv.
        if server_recv_rdy {
            server.recv();
        }

        // Server recv.
        if server_timeout_rdy {
            server.on_timeout();
        }

        // Network recv.
        if socket_recv_rdy {
            let bytes_read = stream.try_read(&mut buf).expect("TODO handle IO error");

            // network task: recv
            network_handle.push_recv_bytes(buf[..bytes_read].to_vec());
        }

        // Network send.
        if network_send_rdy && socket_send_rdy {
            if let Some(send_bytes) = network_handle.get_send() {
                let total_bytes = send_bytes.len();
                let mut already_sent = 0;

                while already_sent < send_bytes.len() {
                    already_sent += stream
                        .try_write(&send_bytes[already_sent..total_bytes])
                        .expect("TODO handle IO error");
                }
            }
        }
    }
}
