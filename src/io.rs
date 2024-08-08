use bytes::Bytes;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub trait Io {
    fn recv(&mut self) -> Option<Bytes>;

    fn send(&mut self, data: Bytes);
}

/// A VecDeque backed IO buffer
#[derive(Default)]
pub struct BufferIo {}

impl BufferIo {
    pub fn split(self) -> (Consumer, Producer) {
        let in_buf = Arc::new(Mutex::new(VecDeque::with_capacity(1024)));
        let out_buf = Arc::new(Mutex::new(VecDeque::with_capacity(1024)));
        let c = Consumer {
            in_buf: out_buf.clone(),
            out_buf: in_buf.clone(),
        };
        let p = Producer { in_buf, out_buf };
        (c, p)
    }
}

/// A handle to the underlying BufferIO held by the network interface
pub struct Consumer {
    in_buf: Arc<Mutex<VecDeque<Bytes>>>,
    out_buf: Arc<Mutex<VecDeque<Bytes>>>,
}

/// A handle to the underlying BufferIO held by the server process
pub struct Producer {
    in_buf: Arc<Mutex<VecDeque<Bytes>>>,
    out_buf: Arc<Mutex<VecDeque<Bytes>>>,
}

impl Io for Producer {
    fn recv(&mut self) -> Option<Bytes> {
        self.in_buf.lock().unwrap().pop_front()
    }

    fn send(&mut self, data: Bytes) {
        self.out_buf.lock().unwrap().push_back(data);
    }
}

impl Io for Consumer {
    fn recv(&mut self) -> Option<Bytes> {
        self.in_buf.lock().unwrap().pop_front()
    }

    fn send(&mut self, data: Bytes) {
        self.out_buf.lock().unwrap().push_back(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producer_consumer() {
        let buf = BufferIo::default();
        let (mut consumer, mut producer) = buf.split();

        producer.send(Bytes::from_static(&[1]));
        producer.send(Bytes::from_static(&[2]));
        producer.send(Bytes::from_static(&[3]));
        consumer.send(Bytes::from_static(&[5]));
        consumer.send(Bytes::from_static(&[6]));
        consumer.send(Bytes::from_static(&[7]));

        assert_eq!(consumer.recv(), Some(Bytes::from_static(&[1])));
        assert_eq!(consumer.recv(), Some(Bytes::from_static(&[2])));
        assert_eq!(consumer.recv(), Some(Bytes::from_static(&[3])));
        assert_eq!(consumer.recv(), None);

        assert_eq!(producer.recv(), Some(Bytes::from_static(&[5])));
        assert_eq!(producer.recv(), Some(Bytes::from_static(&[6])));
        assert_eq!(producer.recv(), Some(Bytes::from_static(&[7])));
        assert_eq!(producer.recv(), None);
    }
}
