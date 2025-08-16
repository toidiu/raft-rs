use crate::{
    io::{BufferIo, NetworkIoImpl, ServerEgressImpl, ServerIngress, ServerIngressImpl},
    mode::{self, Mode},
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
    server_id: ServerId,
    mode: Mode,
    state: RaftState,
    peer_list: Vec<PeerInfo>,
    timer: Timeout,
    io_ingress: ServerIngressImpl,
    io_egress: ServerEgressImpl,
}

impl Server {
    fn new(
        server_id: ServerId,
        peer_list: Vec<PeerInfo>,
        election_timeout: Timeout,
    ) -> (Server, NetworkIoImpl) {
        let (server_io_ingress, server_io_egress, network_io) = BufferIo::split();
        let mode = Mode::new();
        let state = RaftState::new(election_timeout.clone(), &peer_list);
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
        self.state.on_timeout(&mut self.io_egress);
    }

    fn id(&self) -> ServerId {
        self.server_id
    }

    pub fn recv(&mut self) {
        if let Some(mut recv_rpc) = self.io_ingress.recv_rpc() {
            while let Some(rpc) = recv_rpc.next() {
                self.state.on_recv(rpc, &mut self.io_egress);
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

        println!(
            "============== timeout_fut: {} recv_fut: {}",
            timeout_rdy, recv_rdy
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
        io::NetIngress,
        log::{self, Idx, TermIdx},
        rpc::Rpc,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;
    use s2n_codec::{EncoderBuffer, EncoderValue};

    #[tokio::test]
    async fn send_recv() {
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng);
        let server_id = ServerId::new([1; 16]);
        let peer_list = vec![ServerId::new([2; 16]), ServerId::new([3; 16])];
        let peer_list: Vec<PeerInfo> = peer_list.into_iter().map(|x| PeerInfo::new(x)).collect();
        let (server, mut rx_network_io) = Server::new(server_id, peer_list.clone(), timeout);
        let _tx_network_io = rx_network_io.clone();

        let term_initial = log::Term::initial();
        assert_eq!(server.state.current_term, term_initial);

        let mut term_one = log::Term::initial();
        term_one.increment();

        // network: simulate receiving a message from the network
        let mut slice = vec![0; 100];
        let mut buf = EncoderBuffer::new(&mut slice);
        let last_log_term_idx = TermIdx::new(8, 1);
        Rpc::new_request_vote(term_one, peer_list[0].id, last_log_term_idx).encode(&mut buf);
        let (written, buf) = buf.split_mut();
        rx_network_io.recv(written.to_vec());

        let mut buf = EncoderBuffer::new(buf);
        Rpc::new_append_entry(
            term_one,
            server_id,
            TermIdx::new(3, 1),
            Idx::initial(),
            vec![],
        )
        .encode(&mut buf);
        rx_network_io.recv(buf.as_mut_slice().to_vec());

        // server: recv data and queue and data to send
        // server.recv();
        //
        // // network: check data to send out to the network
        // let bytes = tx_network_io.send().unwrap();
        // let buffer = DecoderBuffer::new(&bytes);
        // let (rpc, buffer) = Rpc::decode(buffer).unwrap();
        // let _rpc = cast_unsafe!(rpc, Rpc::RequestVoteResp);
        //
        // let (rpc, buffer) = Rpc::decode(buffer).unwrap();
        // let _rpc = cast_unsafe!(rpc, Rpc::AppendEntryResp);
        //
        // // only 2 responses sent
        // assert!(Rpc::decode(buffer).is_err());
        //
        // assert_eq!(server.state.current_term, term_one);
    }
}
