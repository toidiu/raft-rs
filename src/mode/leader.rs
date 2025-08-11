#[derive(Debug, Default)]
pub struct Leader;

impl Leader {
    pub fn on_leader(&mut self) {
        todo!()
    }

    pub fn on_timeout(&mut self) {
        todo!()
    }

    pub fn on_recv(&mut self, _rpc: crate::rpc::Rpc) {
        todo!()
    }
}
