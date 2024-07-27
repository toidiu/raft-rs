#![allow(unused)]

use crate::server::Server;
use crate::clock::{Clock};
use core::time::Duration;
use tokio::time::sleep;

mod server;
mod clock;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let clock = Clock::default();
    let mut server = Server::new(1);

    loop {
        // recv Commands from peers.
        server.recv();

        // check if timeout elapsed and attempt to make progress
        server.on_timeout();

        // TODO: recv client request
    }
}

