use crate::{io::ServerEgress, mode::Context};

#[derive(Debug, Default)]
pub struct Leader;

impl Leader {
    pub fn on_leader<E: ServerEgress>(&mut self, _context: &mut Context<E>) {
        todo!()
    }

    pub fn on_timeout<E: ServerEgress>(&mut self, _context: &mut Context<E>) {
        todo!()
    }

    pub fn on_recv<E: ServerEgress>(&mut self, _rpc: crate::rpc::Rpc, _context: &mut Context<E>) {
        todo!()
    }
}
