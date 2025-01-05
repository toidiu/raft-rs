use crate::io::ServerTx;

pub struct MockTx;

impl ServerTx for MockTx {
    fn send(&mut self, _data: Vec<u8>) {
        todo!()
    }
}
