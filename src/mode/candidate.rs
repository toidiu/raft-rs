use crate::{
    io::IO_BUF_LEN,
    mode::{Context, ServerTx},
    rpc::Rpc,
};
use s2n_codec::{EncoderBuffer, EncoderValue};

#[derive(Debug, Default)]
pub struct CandidateState;

impl CandidateState {
    pub fn on_candidate<T: ServerTx>(&mut self, tx: &mut T, context: &mut Context) {
        //% Compliance:
        //% On conversion to candidate, start election:
        self.start_election(tx, context);
    }

    pub fn on_timeout<T: crate::io::ServerTx>(&mut self, tx: &mut T, context: &mut Context) {
        //% Compliance:
        //% On conversion to candidate, start election:
        self.start_election(tx, context);
    }

    pub fn on_recv<T: ServerTx>(
        &mut self,
        _tx: &mut T,
        _rpc: crate::rpc::Rpc,
        _state: &mut crate::state::State,
    ) {
        todo!()
    }

    fn start_election<T: ServerTx>(&mut self, tx: &mut T, context: &mut Context) {
        //% Compliance:
        //% Increment currentTerm
        context.state.current_term.increment();

        //% Compliance:
        //% Vote for self
        context.state.voted_for = Some(context.server_id);

        //% Compliance:
        //% Reset election timer
        context.state.election_timer.reset();

        //% Compliance:
        //% Send RequestVote RPCs to all other servers
        //
        // FIXME send a RequestVote to all peers
        let mut slice = vec![0; IO_BUF_LEN];
        let mut buf = EncoderBuffer::new(&mut slice);
        let term = context.state.current_term;
        let last_term_idx = context.log.last_term_idx();
        Rpc::new_request_vote(term, context.server_id, last_term_idx).encode_mut(&mut buf);
        tx.send(buf.as_mut_slice().to_vec());
    }
}
