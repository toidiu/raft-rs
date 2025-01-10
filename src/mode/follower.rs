use crate::{
    io::{ServerIO, IO_BUF_LEN},
    mode::Context,
    rpc::Rpc,
};
use s2n_codec::{EncoderBuffer, EncoderValue};

#[derive(Debug, Default)]
pub struct FollowerState;

impl FollowerState {
    pub fn on_follower<IO: ServerIO>(&mut self, _io: &mut IO) {}

    pub fn on_recv<IO: ServerIO>(
        &mut self,
        tx: &mut IO,
        rpc: crate::rpc::Rpc,
        context: &mut Context<IO>,
    ) {
        //% Compliance:
        //% Respond to RPCs from candidates and leaders
        match rpc {
            Rpc::RequestVote(_request_vote_state) => todo!(),
            Rpc::AppendEntries(append_entries_state) => {
                let response = if append_entries_state.term() < context.state.current_term {
                    //% Compliance:
                    //% Reply false if term < currentTerm (§5.1)
                    false
                } else if !context
                    .state
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
            Rpc::RespRequestVote(_) | Rpc::RespAppendEntries(_) => (),
        }
    }
}
