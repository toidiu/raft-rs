use crate::io::{ServerRx, ServerTx};
use core::task::Waker;
use std::collections::VecDeque;

pub struct Io {
    pub rx: VecDeque<Vec<u8>>,
    pub tx: VecDeque<Vec<u8>>,
    pub waker: Option<Waker>,
}

impl Io {
    pub fn new() -> Self {
        Io {
            rx: VecDeque::new(),
            tx: VecDeque::new(),
            waker: None,
        }
    }
}

impl ServerTx for Io {
    fn send(&mut self, data: Vec<u8>) {
        println!("  -------> {:?}", data);
        self.tx.push_back(data)
    }

    fn poll_tx_ready(&mut self, _cx: &mut std::task::Context) -> std::task::Poll<()> {
        unimplemented!()
    }
}

impl ServerRx for Io {
    fn recv(&mut self) -> Option<Vec<u8>> {
        let data = self.rx.pop_front();
        println!("  <------- {:?}", data);
        data
    }

    fn poll_rx_ready(&mut self, _cx: &mut std::task::Context) -> std::task::Poll<()> {
        unimplemented!()
    }
}
