use crate::{
    clock::Clock,
    io::{BufferIo, NetworkIo, ServerIo, ServerRx},
    rpc::Rpc,
    state::{ServerId, State},
};
use core::{future::Future, task::Poll};
use pin_project_lite::pin_project;
use s2n_codec::{DecoderBuffer, DecoderValue};

#[derive(Debug)]
pub struct Server {
    clock: Clock,
    state: State,
    // IO handle to send and receive Rpc messages
    io: ServerIo,
}

impl Server {
    fn new(clock: Clock, server_list: Vec<ServerId>) -> (Server, NetworkIo) {
        let (server_io, network_io) = BufferIo::split();
        (
            Server {
                clock,
                state: State::new(clock, server_list),
                io: server_io,
            },
            network_io,
        )
    }

    pub fn on_timeout(&mut self) {
        self.state.on_timeout(&mut self.io);
    }

    fn id(&self) -> ServerId {
        self.state.inner.id
    }

    pub fn recv(&mut self) {
        if let Some(bytes) = self.io.recv() {
            let mut buf = DecoderBuffer::new(&bytes);
            while !buf.is_empty() {
                let (rpc, buffer) = Rpc::decode(buf).unwrap();
                println!("  server <--- {:?}", rpc);
                buf = buffer;
                self.state.recv(&mut self.io, rpc);
            }
        }
    }

    async fn poll(&mut self) {
        let fut = ServerFut {
            timeout: &mut self.state.inner.timer,
            recv: self.io.rx_ready(),
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

    #[cfg(test)]
    fn send_test_data(&mut self, data: &[u8]) {
        use crate::io::ServerTx;
        self.io.send_raw_bytes(data);
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
        clock::Clock,
        io::{NetRx, NetTx},
        log::{self, TermIdx},
        testing::cast_unsafe,
    };
    use core::{
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };
    use s2n_codec::{EncoderBuffer, EncoderValue};
    use std::sync::Arc;
    use tokio::time::{advance, sleep};

    const END_MARKER: u8 = 128;

    #[tokio::test]
    async fn mock_event_loop() {
        tokio::time::pause();
        let clock = Clock::default();
        let server_list = vec![ServerId::new(), ServerId::new()];
        let (mut server, mut rx_network_io) = Server::new(clock, server_list.clone());

        let wait_complete = Arc::new(AtomicBool::new(true));
        let set_wait = wait_complete.clone();

        // network: check data to send out to the network
        let mut tx_network_io = rx_network_io.clone();
        tokio::spawn(async move {
            while set_wait.load(Ordering::SeqCst) {
                tx_network_io.tx_ready().await;
                tokio::time::advance(Duration::from_millis(10)).await;

                if let Some(bytes) = tx_network_io.send_to_socket() {
                    let mut bytes = DecoderBuffer::new(&bytes);

                    while !bytes.is_empty() {
                        // Should receive either an Rpc or the END_MARKER
                        if let Ok((rpc, buffer)) = Rpc::decode(bytes) {
                            println!("  ---> network {:?}", rpc);
                            bytes = buffer;
                        } else {
                            match bytes.peek_byte(0) {
                                // the server sent the END_MARKER
                                Ok(end_marger) if end_marger == END_MARKER => {
                                    set_wait.store(false, Ordering::Relaxed);
                                    // println!(
                                    //     "-o-o-o-o-o-o-o-o-o-o-o-o-o-o END {:?} {:?}",
                                    //     bytes.peek_byte(0),
                                    //     bytes
                                    // );
                                    break;
                                }
                                _ => panic!("received unexpected bytes {:?}", bytes),
                            }
                        }
                    }
                }
            }
        });

        // network: simulate receiving a message from the network
        tokio::spawn(async move {
            for i in 0..5 {
                advance(Duration::from_millis(30)).await;

                let mut slice = vec![0; 100];
                let mut buf = EncoderBuffer::new(&mut slice);
                let last_log_term_idx = TermIdx::new(8, 1);
                Rpc::new_request_vote(i, server_list[0], last_log_term_idx).encode(&mut buf);
                let (written, buf) = buf.split_mut();
                rx_network_io.recv_from_socket(written.to_vec());

                let mut buf = EncoderBuffer::new(buf);
                Rpc::new_append_entry(i, TermIdx::new(3, 1)).encode(&mut buf);
                rx_network_io.recv_from_socket(buf.as_mut_slice().to_vec());

                sleep(Duration::from_millis(10)).await;
            }
        });

        // server: recv data
        let mut i = 0;
        while clock.elapsed() < Duration::from_secs(1) {
            server.poll().await;
            println!("---{i} elapsed: {:?}", clock.elapsed());
            i += 1;
        }

        // server: send data
        server.send_test_data(&[END_MARKER]);
        while wait_complete.load(Ordering::SeqCst) {
            advance(Duration::from_millis(300)).await;
            // println!("---waiting for finish tag");
            // println!("---{i} elapsed: {:?}", clock.elapsed());
        }

        assert_eq!(server.state.inner.curr_term, log::Term(9));
    }

    #[tokio::test]
    async fn send_recv() {
        let clock = Clock::default();
        let server_list = vec![ServerId::new(), ServerId::new()];
        let (mut server, mut rx_network_io) = Server::new(clock, server_list.clone());
        let mut tx_network_io = rx_network_io.clone();

        assert_eq!(server.state.inner.curr_term, log::Term(0));

        let term = 1;
        // network: simulate receiving a message from the network
        let mut slice = vec![0; 100];
        let mut buf = EncoderBuffer::new(&mut slice);
        let last_log_term_idx = TermIdx::new(8, 1);
        Rpc::new_request_vote(term, server_list[0], last_log_term_idx).encode(&mut buf);
        let (written, buf) = buf.split_mut();
        rx_network_io.recv_from_socket(written.to_vec());

        let mut buf = EncoderBuffer::new(buf);
        Rpc::new_append_entry(term, TermIdx::new(3, 1)).encode(&mut buf);
        rx_network_io.recv_from_socket(buf.as_mut_slice().to_vec());

        // server: recv data and queue and data to send
        server.recv();

        // network: check data to send out to the network
        let bytes = tx_network_io.send_to_socket().unwrap();
        let buffer = DecoderBuffer::new(&bytes);
        let (rpc, buffer) = Rpc::decode(buffer).unwrap();
        let _rpc = cast_unsafe!(rpc, Rpc::RespRequestVote);

        let (rpc, buffer) = Rpc::decode(buffer).unwrap();
        let _rpc = cast_unsafe!(rpc, Rpc::RespAppendEntries);

        // only 2 responses sent
        assert!(Rpc::decode(buffer).is_err());

        assert_eq!(server.state.inner.curr_term, log::Term(1));
    }
}
