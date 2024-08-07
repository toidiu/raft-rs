#![allow(dead_code)]

use crate::{
    clock::{Clock, Timer},
    server::Server,
};
use bytes::Bytes;
use core::time::Duration;
use std::iter::Iterator;

mod clock;
mod io;
mod log;
mod rpc;
mod server;

fn start() {
    // let clock = Clock::default();
    // let mut server = Server::new(1);

    // loop {
    //     // recv Commands from peers.
    //     server.recv();

    //     // check if timeout elapsed and attempt to make progress
    //     server.on_timeout();

    //     // TODO: recv client request
    // }
}
