use crate::{mode::ServerTx, rpc::Rpc};

#[derive(Debug, Default)]
pub struct FollowerState;

impl FollowerState {
    pub fn on_follower<T: ServerTx>(&mut self, _io: &mut T) {
        todo!()
    }

    pub fn on_recv<T: ServerTx>(
        &mut self,
        _tx: &mut T,
        rpc: crate::rpc::Rpc,
        _state: &mut crate::state::State,
    ) {
        match rpc {
            Rpc::RequestVote(_request_vote_state) => todo!(),
            Rpc::RespRequestVote(_resp_request_vote_state) => todo!(),
            Rpc::AppendEntries(_append_entries_state) => todo!(),
            Rpc::RespAppendEntries(_resp_append_entries_state) => todo!(),
        }
    }
}
