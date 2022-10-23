// Multiple sockets can listen on same port (this is how we create a backlog)

use core::cell::UnsafeCell;
use core::future::Future;
use core::task::{Context, Poll};

use embedded_io::asynch::{Read as AsyncRead, Write as AsyncWrite};
use futures_util::future::poll_fn;
use smoltcp::iface::{Interface, SocketHandle};
use smoltcp::socket::TcpSocket;

use super::devices::Enc28j60;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Error {
    Unknown,
}

impl embedded_io::Error for Error {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

pub struct TcpStream<'a> {
    /// The network interface for the ethernet driver
    interface: &'a UnsafeCell<Interface<'a, Enc28j60>>,
    /// Handle to a TCP socket
    handle: SocketHandle,
}

// ===== impl TcpStream =====

impl<'a> TcpStream<'a> {
    pub fn new(
        interface: &'a UnsafeCell<Interface<'a, Enc28j60>>,
        handle: SocketHandle,
    ) -> TcpStream<'a> {
        TcpStream { interface, handle }
    }

    fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize, Error>> {
        unsafe {
            // TODO: Sanity check grabbing this mutably
            let interface = &mut *self.interface.get();
            let socket = interface.get_socket::<TcpSocket>(self.handle);

            match socket.recv_slice(buf) {
                // No data
                Ok(0) => {
                    socket.register_recv_waker(cx.waker());
                    Poll::Pending
                }
                // Data available
                Ok(n) => Poll::Ready(Ok(n)),
                // EOF
                Err(smoltcp::Error::Finished) => Poll::Ready(Ok(0)),
                // Some error
                Err(_) => Poll::Ready(Err(Error::Unknown)),
            }
        }
    }

    fn poll_write(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>> {
        unsafe {
            // TODO: Sanity check grabbing this mutably
            let interface = &mut *self.interface.get();
            let socket = interface.get_socket::<TcpSocket>(self.handle);

            match socket.send_slice(buf) {
                // No data
                Ok(0) => {
                    socket.register_send_waker(cx.waker());
                    Poll::Pending
                }
                // Data written
                Ok(n) => Poll::Ready(Ok(n)),
                // Some error
                Err(_) => Poll::Ready(Err(Error::Unknown)),
            }
        }
    }
}

impl<'a> embedded_io::Io for TcpStream<'a> {
    type Error = Error;
}

impl<'a> AsyncRead for TcpStream<'a> {
    type ReadFuture<'b> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'b;

    fn read<'b>(&'b mut self, buf: &'b mut [u8]) -> Self::ReadFuture<'b> {
        poll_fn(|cx| self.poll_read(cx, buf))
    }
}

impl<'a> AsyncWrite for TcpStream<'a> {
    type WriteFuture<'b> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'b;

    fn write<'b>(&'b mut self, buf: &'b [u8]) -> Self::WriteFuture<'b> {
        poll_fn(|cx| self.poll_write(cx, buf))
    }

    type FlushFuture<'b> = impl Future<Output = Result<(), Self::Error>>
    where
        Self: 'b;

    fn flush<'b>(&'_ mut self) -> Self::FlushFuture<'_> {
        poll_fn(|_| Poll::Ready(Ok(())))
    }
}
