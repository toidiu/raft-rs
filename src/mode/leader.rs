use crate::io::IO_BUF_LEN;
use crate::rpc::Rpc;
use crate::{io::ServerIO, mode::Context};
use s2n_codec::EncoderBuffer;
use s2n_codec::EncoderValue;

#[derive(Debug, Default)]
pub struct Leader;

impl Leader {
    pub fn on_leader<IO: ServerIO>(&mut self, context: &mut Context<IO>) {
        let current_term = context.state.current_term;
        let self_id = context.server_id;
        let commit_idx = context.state.commit_idx;

        //% Compliance:
        //% Upon election: send initial empty AppendEntries RPCs (heartbeat) to each server; repeat
        //% during idle periods to prevent election timeouts (§5.2)
        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        for (_id, peer) in context.peer_map.iter_mut() {
            //% Compliance:
            //% prevLogIndex: index of log entry immediately preceding new ones
            //% prevLogTerm: term of prevLogIndex entry
            let prev_log_term_idx = context.state.log.last_term_idx();
            Rpc::new_append_entry(current_term, self_id, prev_log_term_idx, commit_idx, vec![])
                .encode_mut(&mut buf);
            peer.send(buf.as_mut_slice().to_vec());
        }
    }

    pub fn on_timeout<IO: ServerIO>(&mut self, _context: &mut Context<IO>) {
        todo!()
    }

    pub fn on_recv<IO: ServerIO>(&mut self, _rpc: crate::rpc::Rpc, _context: &mut Context<IO>) {
        todo!()
    }
}
