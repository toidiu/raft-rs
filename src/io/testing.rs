use crate::io::{ServerRx, ServerTx};

#[derive(Debug)]
pub struct MockTx {
    pub queue: Vec<Vec<u8>>,
}

impl MockTx {
    pub fn new() -> Self {
        MockTx { queue: vec![] }
    }
}

impl ServerTx for MockTx {
    fn send(&mut self, data: Vec<u8>) {
        self.queue.push(data);
    }
}

impl ServerRx for MockTx {
    fn recv(&mut self) -> Option<Vec<u8>> {
        todo!()
    }

    fn poll_rx_ready(&mut self, _cx: &mut std::task::Context) -> std::task::Poll<()> {
        todo!()
    }
}
