use bytes::Bytes;
use core::{
    future::Future,
    task::{Context, Poll},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub trait Io: Sized {
    fn status(&self) {}

    fn recv(&mut self) -> Option<Bytes>;

    fn send(&mut self, data: Bytes);

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to check if there are messages in the receive queue
    //
    // TX is invoked on timeout so we only need to poll the recv queue
    fn rx_ready(&mut self) -> RxReady<Self> {
        RxReady(self)
    }
}

// A handle to check the readiness of the receive queue
pub struct RxReady<'a, T: ?Sized>(&'a mut T);

impl<'a, T: Io> Future for RxReady<'a, T> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.0.poll_ready(cx)
    }
}

/// A VecDeque backed IO buffer
#[derive(Default)]
pub struct BufferIo {}

impl BufferIo {
    pub fn split(self) -> (Consumer, Producer) {
        //   Producer                    Consumer
        //    Server  <- recv [__rx__]  <- send  NetworkInterface/Socket
        //            send -> [__tx__]  -> recv
        let rx = Arc::new(Mutex::new(VecDeque::with_capacity(1024)));
        let tx = Arc::new(Mutex::new(VecDeque::with_capacity(1024)));

        let p = Producer {
            rx: rx.clone(),
            tx: tx.clone(),
        };
        let c = Consumer { rx: tx, tx: rx };
        (c, p)
    }
}

/// Held by the Network interface.
///
/// A handle to the underlying BufferIO
pub struct Consumer {
    rx: Arc<Mutex<VecDeque<Bytes>>>,
    tx: Arc<Mutex<VecDeque<Bytes>>>,
}

/// Held by the Server process.
///
/// A handle to the underlying BufferIO
pub struct Producer {
    rx: Arc<Mutex<VecDeque<Bytes>>>,
    tx: Arc<Mutex<VecDeque<Bytes>>>,
}

impl Io for Producer {
    fn recv(&mut self) -> Option<Bytes> {
        self.rx.lock().unwrap().pop_front()
    }

    fn send(&mut self, data: Bytes) {
        self.tx.lock().unwrap().push_back(data);
    }

    // TX is invoked on timeout so we only need to poll the recv queue
    fn poll_ready(&mut self, _cx: &mut Context) -> Poll<()> {
        let rx_rdy = !self.rx.lock().unwrap().is_empty();
        if rx_rdy {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }

    fn status(&self) {
        let rx = self.rx.lock().unwrap().len();
        let tx = self.tx.lock().unwrap().len();
        println!("------------- check rx: {}  tx: {}", rx, tx);
    }
}

impl Io for Consumer {
    fn recv(&mut self) -> Option<Bytes> {
        self.rx.lock().unwrap().pop_front()
    }

    fn send(&mut self, data: Bytes) {
        self.tx.lock().unwrap().push_back(data);
    }

    // TX is invoked on timeout so we only need to poll the recv queue
    fn poll_ready(&mut self, _cx: &mut Context) -> Poll<()> {
        // let rx_rdy = !self.rx.lock().unwrap().is_empty();
        // if rx_rdy {
        //     Poll::Ready(())
        // } else {
        //     Poll::Pending
        // }
        Poll::Ready(())
    }

    fn status(&self) {
        let rx = self.rx.lock().unwrap().len();
        let tx = self.tx.lock().unwrap().len();
        println!("------------- check rx: {}  tx: {}", rx, tx);
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
