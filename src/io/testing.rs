use crate::io::{Rx, Tx};
use bytes::Bytes;
use core::task::Waker;
use std::collections::VecDeque;

pub struct MockIo {
    pub rx: VecDeque<Bytes>,
    pub tx: VecDeque<Bytes>,
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

impl Tx for MockIo {
    fn send(&mut self, data: Bytes) {
        self.tx.push_back(data)
    }

    fn poll_ready(&mut self, _cx: &mut std::task::Context) -> std::task::Poll<()> {
        unimplemented!()
    }
}

impl Rx for MockIo {
    fn recv(&mut self) -> Option<Bytes> {
        self.rx.pop_front()
    }

    fn poll_ready(&mut self, _cx: &mut std::task::Context) -> std::task::Poll<()> {
        unimplemented!()
    }
}
