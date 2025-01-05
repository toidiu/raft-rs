use crate::{
    io::ServerTx,
    mode::{candidate::CandidateState, follower::FollowerState, leader::LeaderState},
    rpc::Rpc,
};

mod candidate;
mod follower;
mod leader;

enum Mode {
    Follower(FollowerState),
    Candidate(CandidateState),
    Leader(LeaderState),
}

trait Action {
    fn on_convert<T: ServerTx>(&mut self, io: &mut T);

    fn on_timeout<T: ServerTx>(&mut self, io: &mut T);

    fn on_recv<T: ServerTx>(&mut self, io: &mut T, rpc: Rpc);
}

impl Action for Mode {
    fn on_convert<T: ServerTx>(&mut self, io: &mut T) {
        match self {
            Mode::Follower(follower) => follower.on_convert(io),
            Mode::Candidate(candidate) => candidate.on_convert(io),
            Mode::Leader(_leader) => todo!(),
        }
    }

    fn on_timeout<T: ServerTx>(&mut self, io: &mut T) {
        match self {
            Mode::Follower(_follower) => {
                //% Compliance:
                //% If election timeout elapses without receiving AppendEntries RPC from current
                //% leader or granting vote to candidate: convert to candidate
                *self = Mode::Candidate(CandidateState);
                self.on_convert(io);
            }
            Mode::Candidate(candidate) => candidate.on_timeout(io),
            Mode::Leader(_leader) => todo!(),
        }
    }

    fn on_recv<T: ServerTx>(&mut self, _io: &mut T, _rpc: Rpc) {}
}
