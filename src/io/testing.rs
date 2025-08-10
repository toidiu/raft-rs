use crate::io::{ServerTx, IO_BUF_LEN};
use core::task::Waker;
use s2n_codec::{EncoderBuffer, EncoderValue};
use std::collections::VecDeque;

pub struct Io {
    pub rx: VecDeque<Vec<u8>>,
    pub tx: VecDeque<Vec<u8>>,
    pub waker: Option<Waker>,
}

impl Io {
    pub fn new() -> Self {
        Io {
            rx: VecDeque::new(),
            tx: VecDeque::new(),
            waker: None,
        }
    }
}

impl ServerTx for Io {
    fn send_rpc(&mut self, mut rpc: crate::rpc::Rpc) {
        let mut buf = [0; IO_BUF_LEN];

        let mut buf = EncoderBuffer::new(&mut buf);
        rpc.encode_mut(&mut buf);

        let data = buf.as_mut_slice().to_vec();
        println!("  -------> {:?}", data);
        self.tx.push_back(data)
    }
}
