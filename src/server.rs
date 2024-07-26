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
    fn send_test_data(&mut self, data: Vec<u8>) {
        use crate::io::ServerTx;
        // println!("---send test data: {:?}", data);
        self.io.send(data);
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
        log::TermIdx,
    };
    use core::{
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };
    use s2n_codec::{EncoderBuffer, EncoderValue};
    use std::sync::Arc;
    use tokio::time::advance;

    const END_MARKER: u8 = 128;

    #[tokio::test]
    async fn mock_event_loop() {
        tokio::time::pause();
        let clock = Clock::default();
        let server_list = vec![ServerId::new(), ServerId::new()];
        let (mut server, mut network_io) = Server::new(clock, server_list.clone());

        let wait_complete = Arc::new(AtomicBool::new(true));
        let set_wait = wait_complete.clone();

        // network: simulate sending a message
        let mut tx_network_io = network_io.clone();
        tokio::spawn(async move {
            while set_wait.load(Ordering::SeqCst) {
                tx_network_io.tx_ready().await;

                tokio::time::advance(Duration::from_millis(10)).await;
                if let Some(bytes) = tx_network_io.send() {
                    let mut bytes = DecoderBuffer::new(&bytes);
                    while !bytes.is_empty() {
                        if let Ok((rpc, buffer)) = Rpc::decode(bytes) {
                            println!("  ---> network {:?}", rpc);
                            bytes = buffer;
                        } else {
                            if let Ok(end_marker) = bytes.peek_byte(0) {
                                if end_marker == END_MARKER {
                                    set_wait.store(false, Ordering::Relaxed);
                                    // println!(
                                    //     "-o-o-o-o-o-o-o-o-o-o-o-o-o-o END {:?} {:?}",
                                    //     bytes.peek_byte(0),
                                    //     bytes
                                    // );
                                    break;
                                }
                            }

                            panic!("received unexpected bytes {:?}", bytes);
                        }
                    }
                }
            }
        });

        // network: simulate receiving a message over the network
        tokio::spawn(async move {
            for i in 0..5 {
                advance(Duration::from_millis(30)).await;

                let mut slice = vec![0; 100];
                let mut buf = EncoderBuffer::new(&mut slice);
                let last_log_term_idx = TermIdx::new(8, 1);
                Rpc::new_request_vote(i, server_list[0], last_log_term_idx).encode(&mut buf);
                let (written, buf) = buf.split_mut();
                network_io.recv(written.to_vec());

                let mut buf = EncoderBuffer::new(buf);
                Rpc::new_append_entry(i, TermIdx::new(3, 1)).encode(&mut buf);
                network_io.recv(buf.as_mut_slice().to_vec());
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
        server.send_test_data(vec![END_MARKER]);
        while wait_complete.load(Ordering::SeqCst) {
            advance(Duration::from_millis(300)).await;
            // println!("---waiting for finish tag");
            // println!("---{i} elapsed: {:?}", clock.elapsed());
        }
    }
}
