use crate::{io::ServerIO, mode::Context};

#[derive(Debug, Default)]
pub struct Leader;

impl Leader {
    pub fn on_leader<IO: ServerIO>(&mut self, _context: &mut Context<IO>) {
        todo!()
    }

    pub fn on_timeout<IO: ServerIO>(&mut self, _context: &mut Context<IO>) {
        todo!()
    }

    pub fn on_recv<IO: ServerIO>(&mut self, _rpc: crate::rpc::Rpc, _context: &mut Context<IO>) {
        todo!()
    }
}
