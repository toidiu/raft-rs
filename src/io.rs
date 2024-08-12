use bytes::Bytes;
use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
use std::{
    collections::VecDeque,
    ops::Deref,
    sync::{Arc, Mutex},
};

pub trait Io {
    fn recv(&mut self) -> Option<Bytes>;

    fn send(&mut self, data: Bytes);

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to check if there are messages in the receive queue
    //
    // TX is invoked on timeout so we only need to poll the recv queue
    fn io_ready(&mut self) -> IoReady<Self> {
        IoReady(self)
    }

    fn debug_status(&self) {}
}

// A handle to check the readiness of the rx/tx queue.
//
// While all types have an implicit `Sized` by default, traits are
// `?Sized` by default. The ?Sized marker tells the compiler that
// it is fine for T to be potentially not Sized. Alternatively we
// could also have marked `Io: Sized`
pub struct IoReady<'a, T: ?Sized>(&'a mut T);

impl<'a, T: Io> Future for IoReady<'a, T> {
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
        let waker = Arc::new(Mutex::new(None));

        let p = Producer {
            rx: rx.clone(),
            tx: tx.clone(),
            waker: waker.clone(),
        };
        let c = Consumer {
            rx: tx,
            tx: rx,
            waker,
        };
        (c, p)
    }
}

/// Held by the Network interface.
///
/// A handle to the underlying BufferIO
pub struct Consumer {
    rx: Arc<Mutex<VecDeque<Bytes>>>,
    tx: Arc<Mutex<VecDeque<Bytes>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

/// Held by the Server process.
///
/// A handle to the underlying BufferIO
pub struct Producer {
    rx: Arc<Mutex<VecDeque<Bytes>>>,
    tx: Arc<Mutex<VecDeque<Bytes>>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl Io for Producer {
    fn recv(&mut self) -> Option<Bytes> {
        if let Some(waker) = self.waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
        self.rx.lock().unwrap().pop_front()
    }

    fn send(&mut self, data: Bytes) {
        if let Some(waker) = self.waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
        self.tx.lock().unwrap().push_back(data);
    }

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<()> {
        let rdy = !self.rx.lock().unwrap().is_empty() || !self.tx.lock().unwrap().is_empty();
        *self.waker.lock().unwrap() = Some(cx.waker().clone());
        if rdy {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }

    fn debug_status(&self) {
        let rx = self.rx.lock().unwrap().len();
        let tx = self.tx.lock().unwrap().len();
        println!("------------- check rx: {}  tx: {}", rx, tx);
    }
}

impl Io for Consumer {
    fn recv(&mut self) -> Option<Bytes> {
        if let Some(waker) = self.waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
        self.rx.lock().unwrap().pop_front()
    }

    fn send(&mut self, data: Bytes) {
        if let Some(waker) = self.waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
        self.tx.lock().unwrap().push_back(data);
    }

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<()> {
        let rdy = !self.rx.lock().unwrap().is_empty() || !self.tx.lock().unwrap().is_empty();
        *self.waker.lock().unwrap() = Some(cx.waker().clone());
        if rdy {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }

    fn debug_status(&self) {
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
