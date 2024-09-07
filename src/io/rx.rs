use crate::io::{NetworkIo, ServerIo};
use core::{
    future::Future,
    task::{Context, Poll},
};
use std::io::Read;

pub trait Rx {
    fn pop(&mut self) -> Option<Vec<u8>>;

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<()>;

    // A handle to a Future to check for new messages
    fn rx_ready(&mut self) -> RxReady<Self> {
        RxReady(self)
    }
}

// A handle to check the readiness of the rx queue.
//
// While all types have an implicit `Sized` by default, traits are
// `?Sized` by default. The ?Sized marker tells the compiler that
// it is fine for T to be potentially not Sized. Alternatively we
// could also have marked `Rx: Sized`
pub struct RxReady<'a, T: ?Sized>(&'a mut T);

impl<'a, T: Rx> Future for RxReady<'a, T> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.0.poll_ready(cx)
    }
}

impl Rx for NetworkIo {
    fn pop(&mut self) -> Option<Vec<u8>> {
        let mut buf = [0; 100];
        let len = self.rx.lock().unwrap().read(&mut buf[0..]).ok()?;
        if len > 0 {
            Some(buf[0..len].to_vec())
        } else {
            None
        }
    }

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<()> {
        let rdy = !self.rx.lock().unwrap().is_empty();
        *self.waker.lock().unwrap() = Some(cx.waker().clone());
        if rdy {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl Rx for ServerIo {
    fn pop(&mut self) -> Option<Vec<u8>> {
        let mut buf = [0; 100];
        let len = self.rx.lock().unwrap().read(&mut buf[0..]).ok()?;
        if len > 0 {
            Some(buf[0..len].to_vec())
        } else {
            None
        }
    }

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<()> {
        let rdy = !self.rx.lock().unwrap().is_empty();
        *self.waker.lock().unwrap() = Some(cx.waker().clone());
        if rdy {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
