use crate::{
    clock::Clock,
    io, log,
    rpc::Rpc,
    state::{ServerId, State},
};
use core::{future::Future, task::Poll};
use pin_project_lite::pin_project;

#[derive(Debug)]
pub struct Server<T: io::Io> {
    id: ServerId,
    clock: Clock,
    state: State,
    // IO handle to send and receive Rpc messages
    producer: T,

    // ==== persistent state
    // current_term: log::Term,
    // voted_for: Option<ServerId>,
    log: log::Log,
    // # Compliance: Figure 6
    // An entry is considered committed if it is safe for that entry to be applied to state machines.
    //
    // idx of highest log entry known to be committed

    // ==== volatile state
    // commit_idx: u64,
    // idx of the highest log entry applied to the state machine
    // last_applied: u64,
}

impl<T: io::Io> Server<T> {
    fn new(producer: T, clock: Clock) -> Server<T> {
        Server {
            id: ServerId::new(),
            clock,
            state: State::new(clock),
            log: Default::default(),
            producer,
        }
    }

    pub fn on_timeout(&mut self) {
        self.state.on_timeout();
    }

    pub fn recv(&mut self) {
        while let Some(bytes) = self.producer.recv() {
            let rpc = Rpc::try_from(bytes).expect("TODO handle error");
            self.state.recv(rpc);
        }
    }

    async fn poll(&mut self) {
        // FIXME: register the RX future with the runtime
        self.producer.status();
        let fut = ServerFut {
            timeout: self.state.timer(),
            recv: self.producer.rx_ready()
        };

        let Outcome {
            timeout_rdy,
            recv_rdy,
        } = if let Ok(outcome) = fut.await {
            outcome
        } else {
            return;
        };
        self.producer.status();

        println!("==== timeout_fut: {} recv_fut: {}", timeout_rdy, recv_rdy);

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
    use crate::io::Io;
    use crate::io::BufferIo;
    use core::time::Duration;

    #[tokio::test]
    async fn start_server() {
        let (mut c, p) = BufferIo::default().split();
        let clock = Clock::default();
        let mut server = Server::new(p, clock);

        tokio::spawn(async move {
            for i in 0..5 {
                c.send(Rpc::new_request_vote(i).into());
                tokio::time::sleep(Duration::from_millis(200)).await;
                c.send(Rpc::new_append_entry(i).into());
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
