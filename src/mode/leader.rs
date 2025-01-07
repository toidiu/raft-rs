use crate::mode::{Context, ServerTx};

#[derive(Debug, Default)]
pub struct LeaderState;

impl LeaderState {
    pub fn on_leader<T: ServerTx>(&mut self, _io: &mut T) {
        todo!()
    }

    pub fn on_timeout<T: ServerTx>(&mut self, _io: &mut T) {
        todo!()
    }

    pub fn on_recv<T: ServerTx>(
        &mut self,
        _tx: &mut T,
        _rpc: crate::rpc::Rpc,
        _context: &mut Context,
    ) {
        todo!()
    }
}
