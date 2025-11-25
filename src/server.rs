use crate::{
    io::{BufferIo, NetworkIoImpl, ServerEgressImpl, ServerIngress, ServerIngressImpl},
    mode::Mode,
    raft_state::RaftState,
    timeout::Timeout,
};
use pin_project_lite::pin_project;
use std::{future::Future, task::Poll};

mod id;
mod peer;

pub use id::ServerId;
pub use peer::PeerInfo;

struct Server {
    // Unique ServerId for this server process.
    server_id: ServerId,

    // The mode of this server process.
    mode: Mode,

    // Common Raft state for this server process.
    state: RaftState,

    // The list of peers participating in the Raft quorum.
    peer_list: Vec<PeerInfo>,

    // Timeout for making progress.
    timer: Timeout,

    // IO ingress handle.
    io_ingress: ServerIngressImpl,

    // IO egress handle.
    io_egress: ServerEgressImpl,
}

impl Server {
    fn new(
        server_id: ServerId,
        peer_list: Vec<PeerInfo>,
        election_timeout: Timeout,
    ) -> (Server, NetworkIoImpl) {
        let (server_io_ingress, server_io_egress, network_io) = BufferIo::split(server_id);
        let mode = Mode::new();
        let state = RaftState::new(election_timeout.clone());
        let server = Server {
            server_id,
            mode,
            state,
            peer_list,
            timer: election_timeout,
            io_ingress: server_io_ingress,
            io_egress: server_io_egress,
        };

        (server, network_io)
    }

    pub fn on_timeout(&mut self) {
        self.mode.on_timeout(
            &self.server_id,
            &self.peer_list,
            &mut self.state,
            &mut self.io_egress,
        );
    }

    pub fn recv(&mut self) {
        if let Some(recv_rpc) = self.io_ingress.recv_rpc() {
            for packet in recv_rpc {
                let peer_id = packet.from();
                self.mode.on_recv(
                    &self.server_id,
                    peer_id,
                    packet.rpc(),
                    &self.peer_list,
                    &mut self.state,
                    &mut self.io_egress,
                );
            }
        }
    }

    async fn poll(&mut self) {
        let fut = ServerFut {
            timeout: &mut self.timer.timeout_ready(),
            recv: self.io_ingress.ingress_queue_ready(),
        };

        let Outcome {
            timeout_rdy,
            recv_rdy,
        } = if let Ok(outcome) = fut.await {
            outcome
        } else {
            return;
        };

        dbg!(
            "============== timeout_fut: {} recv_fut: {}",
            timeout_rdy,
            recv_rdy
        );

        if timeout_rdy {
            self.on_timeout();
        }
        if recv_rdy {
            self.recv();
        }
    }
}

pin_project! {
    struct ServerFut<S, R> {
        #[pin]
        timeout: S,
        #[pin]
        recv: R
    }
}

struct Outcome {
    timeout_rdy: bool,
    recv_rdy: bool,
}

impl<S, R> Future for ServerFut<S, R>
where
    S: Future,
    R: Future,
{
    type Output = Result<Outcome, ()>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut this = self.project();
        let timeout_rdy = this.timeout.as_mut().poll(cx).is_ready();
        let recv_rdy = this.recv.as_mut().poll(cx).is_ready();

        if timeout_rdy || recv_rdy {
            Poll::Ready(Ok(Outcome {
                timeout_rdy,
                recv_rdy,
            }))
        } else {
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        io::{NetEgress, NetIngress},
        log::{Idx, Term, TermIdx},
        macros::cast_unsafe,
        rpc::{Header, Packet, Rpc},
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;
    use s2n_codec::{DecoderBuffer, DecoderValue, EncoderBuffer, EncoderValue};
    use std::{self, time::Duration};
    use tokio::time::{advance, sleep};

    const TEST_BUF_SIZE: usize = 160;

    // Manually drive state machine.
    // - receive messages on network ingress
    // - received on server (also processes sends)
    // - send messages on network egress
    #[tokio::test]
    async fn send_recv() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng);
        let server_id = ServerId::new([1; 16]);
        let peer_id_2 = ServerId::new([2; 16]);
        let peer_id_3 = ServerId::new([3; 16]);
        let peer_list = vec![peer_id_2, peer_id_3];
        let peer_list: Vec<PeerInfo> = peer_list.into_iter().map(PeerInfo::new).collect();
        let (mut server, mut rx_network_io) = Server::new(server_id, peer_list.clone(), timeout);
        let mut tx_network_io = rx_network_io.clone();

        let term_initial = Term::initial();
        assert_eq!(server.state.current_term, term_initial);
        // send messages with new term
        let term_one = Term::from(1);

        // network ingress:
        // simulate receiving a message from the network
        let mut slice = vec![0; TEST_BUF_SIZE];
        let mut buf = EncoderBuffer::new(&mut slice);
        let last_log_term_idx = TermIdx::builder()
            .with_term(Term::from(8))
            .with_idx(Idx::from(1));
        Packet::new(
            Header {
                from: server_id,
                to: peer_id_2,
            },
            Rpc::new_request_vote(term_one, peer_list[0].id, last_log_term_idx),
        )
        .encode(&mut buf);

        let (written, buf) = buf.split_mut();
        rx_network_io.recv(written.to_vec());

        let mut buf = EncoderBuffer::new(buf);
        Packet::new(
            Header {
                from: server_id,
                to: peer_id_2,
            },
            Rpc::new_append_entry(
                term_one,
                server_id,
                TermIdx::builder()
                    .with_term(Term::from(3))
                    .with_idx(Idx::from(1)),
                Idx::initial(),
                vec![],
            ),
        )
        .encode(&mut buf);

        rx_network_io.recv(buf.as_mut_slice().to_vec());

        // server ingress/egress:
        // trigger the server task. receives data from network + queue data to send
        server.recv();
        assert_eq!(server.state.current_term, term_one);

        // network egress:
        // check data to send out to the network
        let bytes = tx_network_io.send().unwrap();
        let buffer = DecoderBuffer::new(&bytes);
        let (packet, buffer) = Packet::decode(buffer).unwrap();
        let rpc = packet.rpc();
        let _rpc = cast_unsafe!(rpc, Rpc::RequestVoteResp);

        let (packet, buffer) = Packet::decode(buffer).unwrap();
        let rpc = packet.rpc();
        let _rpc = cast_unsafe!(rpc, Rpc::AppendEntryResp);

        // only 2 responses sent
        assert!(Rpc::decode(buffer).is_err());
    }

    // Spawn tokio network tasks and poll server to make progress
    #[tokio::test]
    async fn mock_event_loop() {
        const NUMER_OF_SENDS: u64 = 5;
        // Send 2 messages per round
        const NUMER_OF_MESSAGES: u64 = NUMER_OF_SENDS * 2;

        tokio::time::pause();
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng);
        let server_id = ServerId::new([1; 16]);
        let peer_id_2 = ServerId::new([2; 16]);
        let peer_id_3 = ServerId::new([3; 16]);
        let peer_list = vec![peer_id_2, peer_id_3];
        let peer_list: Vec<PeerInfo> = peer_list.into_iter().map(PeerInfo::new).collect();
        let (mut server, mut rx_network_io) = Server::new(server_id, peer_list.clone(), timeout);
        let mut tx_network_io = rx_network_io.clone();

        // network egress:
        // check data to send out to the network
        tokio::spawn(async move {
            let mut expect_msg_count = NUMER_OF_MESSAGES;

            while expect_msg_count > 0 {
                tx_network_io.tx_ready().await;
                tokio::time::advance(Duration::from_millis(10)).await;

                if let Some(bytes) = tx_network_io.send() {
                    let mut decode_buffer = DecoderBuffer::new(&bytes);

                    while !decode_buffer.is_empty() {
                        // Should receive a Rpc
                        let (_rpc, rem_buffer) = Rpc::decode(decode_buffer).unwrap();
                        decode_buffer = rem_buffer;

                        expect_msg_count -= 1;
                    }
                }
            }
            println!("!!!!!!---------- network egress finish");
        });

        // network ingress:
        // simulate receiving a message from the network
        tokio::spawn(async move {
            for term in 1..=NUMER_OF_SENDS {
                advance(Duration::from_millis(30)).await;

                let mut slice = vec![0; TEST_BUF_SIZE];
                let mut buf = EncoderBuffer::new(&mut slice);
                let last_log_term_idx = TermIdx::builder()
                    .with_term(Term::from(8))
                    .with_idx(Idx::from(1));

                Packet::new(
                    Header {
                        from: server_id,
                        to: peer_id_2,
                    },
                    Rpc::new_request_vote(Term::from(term), peer_list[0].id, last_log_term_idx),
                )
                .encode(&mut buf);

                let (written, buf) = buf.split_mut();
                rx_network_io.recv(written.to_vec());

                let mut buf = EncoderBuffer::new(buf);

                Packet::new(
                    Header {
                        from: server_id,
                        to: peer_id_2,
                    },
                    Rpc::new_append_entry(
                        Term::from(term),
                        peer_list[0].id,
                        TermIdx::builder()
                            .with_term(Term::from(3))
                            .with_idx(Idx::from(1)),
                        Idx::from(1),
                        vec![],
                    ),
                )
                .encode(&mut buf);

                rx_network_io.recv(buf.as_mut_slice().to_vec());

                sleep(Duration::from_millis(10)).await;
            }
            println!("!!!!!!---------- network ingress finish");
        });

        // server ingress/egress:
        // trigger the server task. receives data from network + queue data to send
        for _ in 1..=NUMER_OF_SENDS {
            sleep(Duration::from_millis(10)).await;
            server.poll().await;
        }

        assert_eq!(server.state.current_term, Term::from(5));
    }
}
