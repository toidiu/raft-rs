use crate::mode::{Action, ServerTx};

#[derive(Debug, Default)]
pub struct FollowerState;

impl Action for FollowerState {
    fn on_convert<T: ServerTx>(&mut self, _io: &mut T) {
        todo!()
    }

    fn on_timeout<T: ServerTx>(&mut self, _io: &mut T) {
        unreachable!()
    }

    fn on_recv<T: ServerTx>(&mut self, _io: &mut T, _rpc: crate::rpc::Rpc) {
        todo!()
    }
}
