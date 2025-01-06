use crate::mode::ServerTx;

#[derive(Debug, Default)]
pub struct FollowerState;

impl FollowerState {
    pub fn on_follower<T: ServerTx>(&mut self, _io: &mut T) {
        todo!()
    }

    pub fn on_recv<T: ServerTx>(
        &mut self,
        _tx: &mut T,
        _rpc: crate::rpc::Rpc,
        _state: &mut crate::state::State,
    ) {
    }
}
