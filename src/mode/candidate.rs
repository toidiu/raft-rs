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
        //% If election timeout elapses: start new election
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        io::testing::MockTx, log::TermIdx, mode::Log, server::ServerId, state::State,
        timeout::Timeout,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg32;
    use s2n_codec::DecoderBuffer;

    #[tokio::test]
    async fn test_start_election() {
        let mut candidate = CandidateState;
        let mut tx = MockTx::new();
        let prng = Pcg32::from_seed([0; 16]);
        let timeout = Timeout::new(prng.clone());
        let server_id = ServerId::new([6; 16]);
        let mut state = State::new(timeout);
        let mut current_term = state.current_term;
        let mut context = Context {
            server_id,
            state: &mut state,
            log: &Log::new(),
            peer_list: &vec![],
        };

        candidate.start_election(&mut tx, &mut context);

        // construct RPC to compare
        current_term.increment();
        let expected_rpc = Rpc::new_request_vote(current_term, server_id, TermIdx::initial());

        let rpc_bytes = tx.queue.pop().unwrap();
        let buffer = DecoderBuffer::new(&rpc_bytes);
        let (sent_request_vote, _) = buffer.decode::<Rpc>().unwrap();
        assert_eq!(expected_rpc, sent_request_vote);
    }
}
