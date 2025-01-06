use crate::io::ServerTx;

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
