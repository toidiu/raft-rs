use crate::io::IO_BUF_LEN;
use core::task::Waker;
use std::{
    collections::VecDeque,
    ops::Deref,
    sync::{Arc, Mutex},
};

/// A handle held by the Raft server task.
#[derive(Debug)]
pub struct ServerEgressImpl {
    pub buf: [u8; IO_BUF_LEN],
    pub egress_queue: Arc<Mutex<VecDeque<u8>>>,
    pub egress_waker: Arc<Mutex<Option<Waker>>>,
}

pub trait ServerEgress {
    // Push data to the egress_queue
    fn send(&mut self, data: Vec<u8>);
}

impl ServerEgress for ServerEgressImpl {
    fn send(&mut self, data: Vec<u8>) {
        dbg!("  server ---> {:?}", &data);

        self.egress_queue.lock().unwrap().extend(data);

        if let Some(waker) = self.egress_waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
    }
}
