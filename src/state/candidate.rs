use crate::state::{Action, ServerTx};

#[derive(Debug, Default)]
pub struct CandidateState;

impl CandidateState {
    fn start_election(&mut self) {}
}

impl Action for CandidateState {
    fn on_convert<T: ServerTx>(&mut self, io: &mut T) {
        //% Compliance:
        //% On conversion to candidate, start election:
        self.on_timeout(io);
    }

    fn on_timeout<T: crate::io::ServerTx>(&mut self, _io: &mut T) {
        //% Compliance:
        //% On conversion to candidate, start election:
        self.start_election();
    }

    fn on_recv<T: ServerTx>(&mut self, _io: &mut T, _rpc: crate::rpc::Rpc) {
        todo!()
    }
}
