use core::pin::Pin;
use core::task::{Context, Poll};

use std::io;
use std::net::Shutdown;

use super::addr::ToSocketAddrs;
use crate::io::{pollable::Pollable, AsyncRead, AsyncWrite};

pub struct TcpStream {
    inner: Pollable<std::net::TcpStream>,
}

impl TcpStream {
    pub async fn connect<A: ToSocketAddrs>(addrs: A) -> io::Result<TcpStream> {
        let mut last_err = None;

        for addr in addrs.to_socket_addrs().await? {
            match std::net::TcpStream::connect(addr) {
                Ok(stream) => {
                    stream.set_nonblocking(true)?;
                    let pollable = Pollable::new(stream)?;
                    return Ok(TcpStream { inner: pollable });
                }
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any of the addresses",
            )
        }))
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.inner.poll_read(cx, buf)
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(self.inner.get_ref().shutdown(Shutdown::Write))
    }
}
