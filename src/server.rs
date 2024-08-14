use crate::{
    clock::Clock,
    io::{BufferIo, NetworkIo, Rx, ServerIo},
    rpc::Rpc,
    state::{ServerId, State},
};
use core::{future::Future, task::Poll};
use pin_project_lite::pin_project;

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
        while let Some(bytes) = self.io.recv() {
            let rpc = Rpc::try_from(bytes).expect("TODO handle error");
            self.state.recv(&mut self.io, rpc);
        }
    }

    async fn poll(&mut self) {
        let fut = ServerFut {
            timeout: &mut self.state.common_mut().timer,
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
    use crate::io::Tx;
    use core::time::Duration;

    #[tokio::test]
    async fn mock_event_loop() {
        let clock = Clock::default();
        let (mut server, mut network_io) = Server::new(clock);

        tokio::spawn(async move {
            // simulate receiving a message
            for i in 0..5 {
                network_io.send(Rpc::new_request_vote(i).into());
                tokio::time::sleep(Duration::from_millis(200)).await;
                network_io.send(Rpc::new_append_entry(100 + i).into());
            }
        });

        let mut i = 0;
        while clock.elapsed() < Duration::from_secs(1) {
            server.poll().await;
            println!("---{i} elapsed: {:?}", clock.elapsed());
            i += 1;
        }
    }
}
