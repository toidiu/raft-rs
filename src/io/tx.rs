use crate::io::{NetworkIo, ServerIo};
use bytes::Bytes;
use core::{
    future::Future,
    task::{Context, Poll},
};
use std::ops::Deref;

pub trait Tx {
    fn send(&mut self, data: Bytes);

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to a Future to check for new messages
    fn tx_ready(&mut self) -> TxReady<Self> {
        TxReady(self)
    }
}

// A handle to check the readiness of the tx queue.
//
// While all types have an implicit `Sized` by default, traits are
// `?Sized` by default. The ?Sized marker tells the compiler that
// it is fine for T to be potentially not Sized. Alternatively we
// could also have marked `Tx: Sized`
pub struct TxReady<'a, T: ?Sized>(&'a mut T);

impl<'a, T: Tx> Future for TxReady<'a, T> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.0.poll_ready(cx)
    }
}

impl Tx for NetworkIo {
    fn send(&mut self, data: Bytes) {
        if let Some(waker) = self.waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
        self.tx.lock().unwrap().push_back(data);
    }

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<()> {
        let rdy = !self.tx.lock().unwrap().is_empty();
        *self.waker.lock().unwrap() = Some(cx.waker().clone());
        if rdy {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl Tx for ServerIo {
    fn send(&mut self, data: Bytes) {
        if let Some(waker) = self.waker.lock().unwrap().deref() {
            waker.wake_by_ref();
        }
        self.tx.lock().unwrap().push_back(data);
    }

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<()> {
        let rdy = !self.tx.lock().unwrap().is_empty();
        *self.waker.lock().unwrap() = Some(cx.waker().clone());
        if rdy {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
