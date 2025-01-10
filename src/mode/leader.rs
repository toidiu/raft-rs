use crate::{io::ServerIO, mode::Context};

#[derive(Debug, Default)]
pub struct LeaderState;

impl LeaderState {
    pub fn on_leader<IO: ServerIO>(&mut self, _io: &mut IO) {
        todo!()
    }

    pub fn on_timeout<IO: ServerIO>(&mut self, _io: &mut IO) {
        todo!()
    }

    pub fn on_recv<IO: ServerIO>(
        &mut self,
        _tx: &mut IO,
        _rpc: crate::rpc::Rpc,
        _context: &mut Context<IO>,
    ) {
        todo!()
    }
}
