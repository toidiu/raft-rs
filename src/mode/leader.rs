use crate::mode::{Action, ServerTx};

#[derive(Debug, Default)]
pub struct LeaderState {}

impl Action for LeaderState {
    fn on_convert<T: ServerTx>(&mut self, _io: &mut T) {
        todo!()
    }

    fn on_timeout<T: ServerTx>(&mut self, _io: &mut T) {
        todo!()
    }

    fn on_recv<T: ServerTx>(&mut self, _io: &mut T, _rpc: crate::rpc::Rpc) {
        todo!()
    }
}
