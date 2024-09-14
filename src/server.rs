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
    id: ServerId,
    clock: Clock,
    state: State,
    // IO handle to send and receive Rpc messages
    io: ServerIo,
}

impl Server {
    fn new(clock: Clock) -> (Server, NetworkIo) {
        let (server_io, network_io) = BufferIo::split();
        (
            Server {
                id: ServerId::new(),
                clock,
                state: State::new(clock),
                io: server_io,
            },
            network_io,
        )
    }

    pub fn on_timeout(&mut self) {
        self.state.on_timeout(&mut self.io);
    }

    pub fn recv(&mut self) {
        if let Some(bytes) = self.io.recv() {
            let mut buf = DecoderBuffer::new(&bytes);
            while !buf.is_empty() {
                let (rpc, buffer) = Rpc::decode(buf).unwrap();
                println!("--- rpc: {:?}", rpc);
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
    #[tokio::test]
    async fn mock_event_loop() {
        let clock = Clock::default();
        let (mut server, mut network_io) = Server::new(clock);

        let wait_complete = Arc::new(AtomicBool::new(true));
        let set_wait = wait_complete.clone();

        // network: simulate sending a message
        let mut tx_network_io = network_io.clone();
        tokio::spawn(async move {
            loop {
                tx_network_io.tx_ready().await;
                tokio::time::advance(Duration::from_millis(100)).await;
                if let Some(bytes) = tx_network_io.send() {
                    println!("---send {:?}", bytes);
                    // TODO currently we read the entire set of available bytes. instead read
                    // only a single packet. RPC should contain TAG/LEN
                    if bytes.contains(&128) {
                        set_wait.store(false, Ordering::Relaxed);
                        break;
                    }

                    let mut buf = DecoderBuffer::new(&bytes);
                    while !buf.is_empty() {
                        if let Ok((rpc, buffer)) = Rpc::decode(buf) {
                            println!("---send {:?}", rpc);
                            buf = buffer;
                        } else {
                            break;
                        }
                    }
                }
            }
        });

        // network: simulate receiving a message over the network
        tokio::spawn(async move {
            for i in 0..5 {
                let mut slice = vec![0; 100];

                let mut buf = EncoderBuffer::new(&mut slice);
                Rpc::new_request_vote(i).encode(&mut buf);
                let (written, buf) = buf.split_mut();
                network_io.recv(written.to_vec());

                advance(Duration::from_millis(200)).await;

                let mut buf = EncoderBuffer::new(buf);
                Rpc::new_append_entry(i + 100, TermIdx::new(3, 1)).encode(&mut buf);
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
        server.send_test_data(vec![128]);
        while wait_complete.load(Ordering::SeqCst) {
            advance(Duration::from_millis(100)).await;
            println!("---waiting for finish tag");
        }
    }
}
