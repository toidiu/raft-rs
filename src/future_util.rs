use pin_project_lite::pin_project;
use std::{future::Future, task::Poll};

pin_project! {
    pub(crate) struct EventSelectFuture<A, B, C, D, E> {
        #[pin]
        pub server_recv: A,
        #[pin]
        pub server_timeout: B,
        #[pin]
        pub network_send: C,
        #[pin]
        pub socket_send: D,
        #[pin]
        pub socket_recv: E,
    }
}

pub(crate) struct EventSelectOutcome {
    pub server_recv_rdy: bool,
    pub server_timeout_rdy: bool,
    pub network_send_rdy: bool,
    pub socket_send_rdy: bool,
    pub socket_recv_rdy: bool,
}

impl<A, B, C, D, E> Future for EventSelectFuture<A, B, C, D, E>
where
    A: Future,
    B: Future,
    C: Future,
    D: Future,
    E: Future,
{
    type Output = EventSelectOutcome;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut this = self.project();
        let server_recv_rdy = this.server_recv.as_mut().poll(cx).is_ready();
        let server_timeout_rdy = this.server_timeout.as_mut().poll(cx).is_ready();
        let network_send_rdy = this.network_send.as_mut().poll(cx).is_ready();
        let socket_send_rdy = this.socket_send.as_mut().poll(cx).is_ready();
        let socket_recv_rdy = this.socket_recv.as_mut().poll(cx).is_ready();

        if server_recv_rdy
            || server_timeout_rdy
            || network_send_rdy
            || socket_send_rdy
            || socket_recv_rdy
        {
            Poll::Ready(EventSelectOutcome {
                server_recv_rdy,
                server_timeout_rdy,
                network_send_rdy,
                socket_send_rdy,
                socket_recv_rdy,
            })
        } else {
            Poll::Pending
        }
    }
}
