use crate::io::Tx;
use crate::{
    clock::Clock,
    io::{BufferIo, NetworkIo, Rx, ServerIo},
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
        if let Some(bytes) = self.io.pop() {
            let mut buf = DecoderBuffer::new(&bytes);

            while !buf.is_empty() {
                let (rpc, buffer) = Rpc::decode(buf).expect("todo");
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
        println!("1-----------32");
        self.io.push(data);
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
    use core::sync::atomic::AtomicBool;
    use core::sync::atomic::Ordering;
    use core::time::Duration;
    use s2n_codec::{EncoderBuffer, EncoderValue};
    use std::sync::Arc;

    #[tokio::test]
    async fn mock_event_loop() {
        let clock = Clock::default();
        let (mut server, mut network_io) = Server::new(clock);

        let wait_complete = Arc::new(AtomicBool::new(true));
        let set_wait = wait_complete.clone();

        // // simulate sending a message over the network.
        let mut tx_network_io = network_io.clone();
        tokio::spawn(async move {
            loop {
                // println!("---bYTES--------------------------- POLL");
                tx_network_io.tx_ready().await;
                // println!("---bYTES--------------------------- POLL laskjfdklasjdflkjak3o832923923");
                tokio::time::sleep(Duration::from_millis(1)).await;
                if let Some(bytes) = tx_network_io.pop() {
                    println!("---bYTES--------------------------- bytes: {:?}", bytes);

                    if bytes == vec![1, 2, 3] {
                        set_wait.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            }
        });

        // simulate receiving a message over the network
        let mut rx_network_io = network_io.clone();
        tokio::spawn(async move {
            for i in 0..5 {
                let mut slice = vec![0; 100];
                let mut buf = EncoderBuffer::new(&mut slice);
                Rpc::new_request_vote(i).encode(&mut buf);
                let (written, rem) = buf.split_mut();
                rx_network_io.push(written.to_vec());

                tokio::time::sleep(Duration::from_millis(200)).await;

                let mut buf = EncoderBuffer::new(rem);
                Rpc::new_append_entry(i + 100).encode(&mut buf);
                rx_network_io.push(buf.as_mut_slice().to_vec());
            }
        });

        let mut i = 0;
        while clock.elapsed() < Duration::from_secs(1) {
            server.poll().await;
            println!("---{i} elapsed: {:?}", clock.elapsed());
            i += 1;
        }

        server.send_test_data(vec![1, 2, 3]);
        tokio::time::sleep(Duration::from_millis(10)).await;

        while wait_complete.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(20)).await;
            println!("---WAIT");
        }
    }
}
