use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use std::io;
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};

// Async version of ToSocketAddrs trait
// TODO: Implement blocking APIs for converting strings into
// SocketAddrs.
pub trait ToSocketAddrs {
    type Iter: Iterator<Item = SocketAddr>;

    fn to_socket_addrs(&self) -> ToSocketAddrsFuture<Self::Iter>;
}

pub enum ToSocketAddrsFuture<I> {
    // Resolving
    Ready(io::Result<I>),
    Done,
}

// TODO: Figure out whether this is actually valid?
// What happens if I is of type !Unpin?
// https://doc.rust-lang.org/nightly/core/pin/index.html#projections-and-structural-pinning
impl<I> Unpin for ToSocketAddrsFuture<I> {}

impl<I: Iterator<Item = SocketAddr>> Future for ToSocketAddrsFuture<I> {
    type Output = io::Result<I>;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        use core::mem;
        let state = mem::replace(&mut *self, ToSocketAddrsFuture::Done);

        match state {
            ToSocketAddrsFuture::Ready(res) => Poll::Ready(res),
            ToSocketAddrsFuture::Done => panic!("Polled completed future"),
        }
    }
}

// ===== impl ToSocketAddrs for SocketAddr[V4/V6]

impl ToSocketAddrs for SocketAddr {
    type Iter = core::option::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> ToSocketAddrsFuture<Self::Iter> {
        let iter = Some(*self).into_iter();
        ToSocketAddrsFuture::Ready(Ok(iter))
    }
}

impl ToSocketAddrs for SocketAddrV4 {
    type Iter = core::option::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> ToSocketAddrsFuture<Self::Iter> {
        let addr = SocketAddr::V4(*self);
        ToSocketAddrs::to_socket_addrs(&addr)
    }
}

impl ToSocketAddrs for SocketAddrV6 {
    type Iter = core::option::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> ToSocketAddrsFuture<Self::Iter> {
        let addr = SocketAddr::V6(*self);
        ToSocketAddrs::to_socket_addrs(&addr)
    }
}
