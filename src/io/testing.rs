use crate::io::{ServerRx, ServerTx};
use core::task::Waker;
use std::collections::VecDeque;

pub struct MockIo {
    pub rx: VecDeque<Vec<u8>>,
    pub tx: VecDeque<Vec<u8>>,
    pub waker: Option<Waker>,
}

impl MockIo {
    pub fn new() -> Self {
        MockIo {
            rx: VecDeque::new(),
            tx: VecDeque::new(),
            waker: None,
        }
    }
}

impl ServerTx for MockIo {
    fn send(&mut self, data: Vec<u8>) {
        self.tx.push_back(data)
    }

    fn poll_tx_ready(&mut self, _cx: &mut std::task::Context) -> std::task::Poll<()> {
        unimplemented!()
    }
}

impl ServerRx for MockIo {
    fn recv(&mut self) -> Option<Vec<u8>> {
        self.rx.pop_front()
    }

    fn poll_rx_ready(&mut self, _cx: &mut std::task::Context) -> std::task::Poll<()> {
        unimplemented!()
    }
}
