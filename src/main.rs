#![allow(unused)]

use crate::{
    clock::{Clock, Timer},
    server::Server,
};
use core::time::Duration;
use std::iter::Iterator;
use tokio::time::sleep;

mod clock;
mod server;

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
