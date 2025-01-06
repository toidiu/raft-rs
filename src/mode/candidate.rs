use crate::{mode::ServerTx, server::ServerId, state::State};

#[derive(Debug, Default)]
pub struct CandidateState;

impl CandidateState {
    pub fn on_candidate<T: ServerTx>(
        &mut self,
        _io: &mut T,
        state: &mut State,
        server_id: ServerId,
    ) {
        //% Compliance:
        //% On conversion to candidate, start election:
        self.start_election(state, server_id);
    }

    pub fn on_timeout<T: crate::io::ServerTx>(
        &mut self,
        _io: &mut T,
        state: &mut State,
        server_id: ServerId,
    ) {
        //% Compliance:
        //% On conversion to candidate, start election:
        self.start_election(state, server_id);
    }

    pub fn on_recv<T: ServerTx>(
        &mut self,
        _tx: &mut T,
        _rpc: crate::rpc::Rpc,
        _state: &mut crate::state::State,
    ) {
        todo!()
    }

    fn start_election(&mut self, state: &mut State, server_id: ServerId) {
        //% Compliance:
        //% Increment currentTerm
        state.current_term.increment();

        //% Compliance:
        //% Vote for self
        state.voted_for = Some(server_id);

        //% Compliance:
        //% Reset election timer
        state.election_timer.reset();

        //% Compliance:
        //% Send RequestVote RPCs to all other servers
        // TODO
    }
}
