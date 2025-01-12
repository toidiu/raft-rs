use crate::{
    io::{ServerIO, IO_BUF_LEN},
    mode::Context,
    rpc::{AppendEntriesState, Rpc},
};
use s2n_codec::{EncoderBuffer, EncoderValue};
use std::cmp::min;

#[derive(Debug, Default)]
pub struct FollowerState;

impl FollowerState {
    pub fn on_follower<IO: ServerIO>(&mut self, _context: &mut Context<IO>) {}

    pub fn on_recv<IO: ServerIO>(&mut self, rpc: crate::rpc::Rpc, context: &mut Context<IO>) {
        //% Compliance:
        //% Respond to RPCs from candidates and leaders
        match rpc {
            Rpc::RequestVote(_request_vote_state) => todo!(),
            Rpc::AppendEntries(append_entries_state) => {
                let AppendEntriesState {
                    term,
                    leader_id,
                    prev_log_term_idx,
                    leader_commit_idx,
                    entries,
                } = append_entries_state;
                let leader_io = &mut context.peer_map.get_mut(&leader_id).unwrap().io;
                let response = if term < context.state.current_term {
                    //% Compliance:
                    //% Reply false if term < currentTerm (§5.1)
                    false
                } else if !context.state.log.entry_matches(prev_log_term_idx) {
                    //% Compliance:
                    //% Reply false if log doesn’t contain an entry at prevLogIndex whose term
                    //% matches prevLogTerm (§5.3)
                    false
                } else {
                    true
                };

                //% Compliance:
                //% If an existing entry conflicts with a new one (same index but different terms),
                //% delete the existing entry and all that follow it (§5.3)
                //
                //% Compliance:
                //% Append any new entries not already in the log
                for entry in entries.into_iter() {
                    context.state.log.match_leaders_log(entry);
                }

                //% Compliance:
                //% If leaderCommit > commitIndex, set commitIndex = min(leaderCommit, index of
                //% last new entry)
                if leader_commit_idx > context.state.commit_idx {
                    context.state.commit_idx = min(leader_commit_idx, context.state.log.prev_idx());
                }

                let mut slice = vec![0; IO_BUF_LEN];
                let mut buf = EncoderBuffer::new(&mut slice);
                let term = context.state.current_term;
                Rpc::new_append_entry_resp(term, response).encode_mut(&mut buf);
                leader_io.send(buf.as_mut_slice().to_vec());
            }
            Rpc::RespRequestVote(_) | Rpc::RespAppendEntries(_) => (),
        }
    }
}
