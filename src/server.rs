use crate::{
    clock::Clock,
    io, log,
    rpc::Rpc,
    state::{ServerId, State},
};
use core::{
    task::{Context, Poll},
    time::Duration,
};

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

    pub fn poll_ready(&mut self, ctx: &mut Context) -> Poll<()> {
        self.state.poll_ready(ctx)
    }

    pub fn recv(&mut self) {
        while let Some(bytes) = self.producer.recv() {
            let rpc = Rpc::try_from(bytes).expect("TODO handle error");
            self.state.recv(rpc);
        }
    }

    async fn start(mut self) {
        let mut i = 0;

        while self.clock.elapsed() < Duration::from_secs(1) {
            // await the timer
            self.state.timer().await;
            self.recv();

            println!("---{i} elapsed: {:?}", self.clock.elapsed());
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{BufferIo, Io};

    #[tokio::test]
    async fn start_server() {
        let (mut c, p) = BufferIo::default().split();
        let server = Server::new(p, Clock::default());

        tokio::spawn(async move {
            for i in 0..5 {
                c.send(Rpc::new_request_vote(i).into());
                tokio::time::sleep(Duration::from_millis(200)).await;
                c.send(Rpc::new_append_entry(i).into());
            }
        });
        server.start().await;
    }
}
