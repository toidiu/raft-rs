#![allow(dead_code)]

mod clock;
mod io;
mod log;
mod rpc;
mod server;
mod state;

#[cfg(test)]
mod testing;

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
