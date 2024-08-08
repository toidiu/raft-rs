use std::{
    collections::VecDeque,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

pub trait IO<T> {
    fn recv(&mut self) -> Option<T>;

    fn send(&mut self, data: T);
}

/// A VecDeque backed IO buffer
#[derive(Default)]
pub struct BufferIO<T> {
    p: PhantomData<T>,
}

impl<T> BufferIO<T> {
    pub fn split(self) -> (Consumer<T>, Producer<T>) {
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
pub struct Consumer<T> {
    in_buf: Arc<Mutex<VecDeque<T>>>,
    out_buf: Arc<Mutex<VecDeque<T>>>,
}

/// A handle to the underlying BufferIO held by the server process
pub struct Producer<T> {
    in_buf: Arc<Mutex<VecDeque<T>>>,
    out_buf: Arc<Mutex<VecDeque<T>>>,
}

impl<T> IO<T> for Producer<T> {
    fn recv(&mut self) -> Option<T> {
        self.in_buf.lock().unwrap().pop_front()
    }

    fn send(&mut self, data: T) {
        self.out_buf.lock().unwrap().push_back(data);
    }
}

impl<T> IO<T> for Consumer<T> {
    fn recv(&mut self) -> Option<T> {
        self.in_buf.lock().unwrap().pop_front()
    }

    fn send(&mut self, data: T) {
        self.out_buf.lock().unwrap().push_back(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producer_consumer() {
        let buf = BufferIO::<u8>::default();
        let (mut consumer, mut producer) = buf.split();

        producer.send(1);
        producer.send(2);
        producer.send(3);
        consumer.send(5);
        consumer.send(6);
        consumer.send(7);

        assert_eq!(consumer.recv(), Some(1));
        assert_eq!(consumer.recv(), Some(2));
        assert_eq!(consumer.recv(), Some(3));
        assert_eq!(consumer.recv(), None);

        assert_eq!(producer.recv(), Some(5));
        assert_eq!(producer.recv(), Some(6));
        assert_eq!(producer.recv(), Some(7));
        assert_eq!(producer.recv(), None);
    }
}
