use core::time::Duration;
use crate::rpc::Rpc;
use crate::{
    clock::Clock,
    io, log,
    state::{ServerId, State},
};
use core::task::{Context, Poll};

#[derive(Debug)]
pub struct Server<T: io::Io> {
    id: ServerId,
    clock: Clock,
    state: State,
    // IO handle to send and receive Rpc messages
    io_producer: T,

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
            io_producer: producer,
        }
    }

    pub fn poll_ready(&mut self, ctx: &mut Context) -> Poll<()> {
        self.state.poll_ready(ctx)
    }

    pub fn recv(&mut self, rpc: Rpc) {
        self.state.recv(rpc);
    }

    async fn start(mut self) {
        let mut i = 0;

        while self.clock.elapsed() < Duration::from_secs(1) {
            // await the timer
             self.state.timer().await;

            println!("---{i} elapsed: {:?}", self.clock.elapsed());
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::BufferIo;

    #[tokio::test]
    async fn start_server() {
        let (_c, p) = BufferIo::default().split();
        let server = Server::new(p, Clock::default());
        server.start().await;
    }
}
