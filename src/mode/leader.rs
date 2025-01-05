use crate::{
    log::Idx,
    mode::{Action, ServerTx},
    server::ServerId,
};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct LeaderState {
    // ==== Volatile state on leaders ====
    //% Compliance
    //% `nextIndex[]` for each server, index of the next log entry to send to that server
    //% (initialized to leader last log index + 1)
    next_idx: BTreeMap<ServerId, Idx>,

    //% Compliance
    //% `matchIndex[]` for each server, index of highest log entry known to be replicated on server
    //% (initialized to 0, increases monotonically)
    match_idx: BTreeMap<ServerId, Idx>,
}

impl Action for LeaderState {
    fn on_convert<T: ServerTx>(&mut self, _io: &mut T) {
        todo!()
    }

    fn on_timeout<T: ServerTx>(&mut self, _io: &mut T) {
        todo!()
    }

    fn on_recv<T: ServerTx>(&mut self, _io: &mut T, _rpc: crate::rpc::Rpc) {
        todo!()
    }
}
