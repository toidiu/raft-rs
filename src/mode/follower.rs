use crate::{
    io::IO_BUF_LEN,
    mode::{Context, ServerTx},
    rpc::Rpc,
};
use s2n_codec::{EncoderBuffer, EncoderValue};

#[derive(Debug, Default)]
pub struct FollowerState;

impl FollowerState {
    pub fn on_follower<T: ServerTx>(&mut self, _io: &mut T) {}

    pub fn on_recv<T: ServerTx>(
        &mut self,
        tx: &mut T,
        rpc: crate::rpc::Rpc,
        context: &mut Context,
    ) {
        //% Compliance:
        //% Respond to RPCs from candidates and leaders
        match rpc {
            crate::rpc::Rpc::RequestVote(_request_vote_state) => todo!(),
            crate::rpc::Rpc::RespRequestVote(_resp_request_vote_state) => todo!(),
            crate::rpc::Rpc::AppendEntries(append_entries_state) => {
                let response = if append_entries_state.term() < context.state.current_term {
                    //% Compliance:
                    //% Reply false if term < currentTerm (§5.1)
                    false
                } else if !context
                    .log
                    .entry_matches(append_entries_state.prev_log_term_idx)
                {
                    //% Compliance:
                    //% Reply false if log doesn’t contain an entry at prevLogIndex whose term  matches prevLogTerm (§5.3)
                    false
                } else {
                    true
                };

                let mut slice = vec![0; IO_BUF_LEN];
                let mut buf = EncoderBuffer::new(&mut slice);
                let term = context.state.current_term;
                Rpc::new_append_entry_resp(term, response).encode_mut(&mut buf);
                tx.send(buf.as_mut_slice().to_vec());
            }
            crate::rpc::Rpc::RespAppendEntries(_resp_append_entries_state) => todo!(),
        }
    }
}
