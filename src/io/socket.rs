use pin_project_lite::pin_project;
use std::{future::Future, task::Poll};
use tokio::net::TcpStream;

macro_rules! socket_ready(
    ($name:ident, $fn:ident) => {
        pin_project! {
            /// A handle to check if socket is ready.
            pub(crate) struct $name<'a> {
                #[pin]
                pub stream: &'a TcpStream,
            }
        }

        impl Future for $name<'_> {
            type Output = ();

            fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
                let mut this = self.project();
                let poll = this.stream.as_mut().$fn(cx);
                if poll.is_ready() {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            }
        }

    };
);

socket_ready!(SocketRecvReady, poll_read_ready);
socket_ready!(SocketSendReady, poll_write_ready);
