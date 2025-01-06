use crate::mode::ServerTx;

#[derive(Debug, Default)]
pub struct CandidateState;

impl CandidateState {
    pub fn on_candidate<T: ServerTx>(&mut self, io: &mut T) {
        //% Compliance:
        //% On conversion to candidate, start election:
        self.on_timeout(io);
    }

    pub fn on_timeout<T: crate::io::ServerTx>(&mut self, _io: &mut T) {
        //% Compliance:
        //% On conversion to candidate, start election:
        self.start_election();
    }

    pub fn on_recv<T: ServerTx>(
        &mut self,
        _tx: &mut T,
        _rpc: crate::rpc::Rpc,
        _state: &mut crate::state::State,
    ) {
        todo!()
    }

    fn start_election(&mut self) {}
}
