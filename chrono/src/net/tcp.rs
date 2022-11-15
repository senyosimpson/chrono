// Multiple sockets can listen on same port (this is how we create a backlog)

use core::fmt;
use core::future::{poll_fn, Future};
use core::task::{Context, Poll};

use smoltcp::iface::SocketHandle;
use smoltcp::socket::{self, TcpSocketBuffer, TcpState};
use smoltcp::wire::IpEndpoint;

use crate::io::{AsyncRead, AsyncWrite};
use crate::net;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Error {
    Unknown,
    AlreadyOpen,
    InvalidPort,
}

impl embedded_io::Error for Error {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unknown => write!(f, "unknown error"),
            Error::AlreadyOpen => write!(f, "socket already open"),
            Error::InvalidPort => write!(f, "invalid port"),
        }
    }
}

pub struct TcpSocket {
    /// Handle to a TCP socket
    handle: SocketHandle,
}

// ===== impl TcpSocket =====

impl TcpSocket {
    pub fn new<'a>(rx_buffer: &'a mut [u8], tx_buffer: &'a mut [u8]) -> TcpSocket {
        // Change the lifetime of the buffers to 'static. It is valid to do this because
        // we know they last for the lifetime of the program.
        let rx_buffer: &'static mut [u8] = unsafe { core::mem::transmute(rx_buffer) };
        let tx_buffer: &'static mut [u8] = unsafe { core::mem::transmute(tx_buffer) };

        let tcp_rx_buffer = TcpSocketBuffer::new(&mut rx_buffer[..]);
        let tcp_tx_buffer = TcpSocketBuffer::new(&mut tx_buffer[..]);
        let socket = socket::TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);

        let mut inner = net::stack().inner.as_ref().unwrap().borrow_mut();
        let handle = inner.interface.add_socket(socket);

        TcpSocket { handle }
    }

    pub fn listen<T>(&self, addr: T) -> Result<(), Error>
    where
        T: Into<IpEndpoint> + Copy,
    {
        let mut inner = net::stack().inner.as_ref().unwrap().borrow_mut();
        let socket = inner.interface.get_socket::<socket::TcpSocket>(self.handle);

        match socket.listen(addr) {
            Ok(_) => Ok(()),
            Err(e) => match e {
                smoltcp::Error::Illegal => Err(Error::AlreadyOpen),
                smoltcp::Error::Unaddressable => Err(Error::InvalidPort),
                _ => unreachable!(),
            },
        }?;

        // Configure socket
        // Keep alive set to ping every 1.25 minutes
        socket.set_keep_alive(Some(smoltcp::time::Duration::from_millis(1000)));

        Ok(())
    }

    pub async fn accept(&self) -> Result<(), Error> {
        poll_fn(|cx| self.poll_accept(cx)).await
    }

    pub fn poll_accept(&self, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let mut inner = net::stack().inner.as_ref().unwrap().borrow_mut();
        let socket = inner.interface.get_socket::<socket::TcpSocket>(self.handle);

        match socket.state() {
            TcpState::Listen | TcpState::SynReceived | TcpState::SynSent => {
                defmt::trace!("Not ready to accept. Registering send waker");
                socket.register_send_waker(cx.waker());
                Poll::Pending
            }
            _ => {
                defmt::trace!("Accepted connection!");
                Poll::Ready(Ok(()))
            }
        }
    }

    fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize, Error>> {
        // TODO: Sanity check grabbing this mutably
        let mut inner = net::stack().inner.as_ref().unwrap().borrow_mut();
        let socket = inner.interface.get_socket::<socket::TcpSocket>(self.handle);

        match socket.recv_slice(buf) {
            // No data
            Ok(0) => {
                defmt::debug!("No data. Pending");
                socket.register_recv_waker(cx.waker());
                Poll::Pending
            }
            // Data available
            Ok(n) => Poll::Ready(Ok(n)),
            // EOF
            Err(smoltcp::Error::Finished) => {
                defmt::debug!("Finished reading!");
                Poll::Ready(Ok(0))
            }
            // Some error
            Err(_) => {
                defmt::trace!("Unknown error");
                Poll::Ready(Err(Error::Unknown))
            }
        }
    }

    fn poll_write(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>> {
        // TODO: Sanity check grabbing this mutably
        let mut inner = net::stack().inner.as_ref().unwrap().borrow_mut();
        let socket = inner.interface.get_socket::<socket::TcpSocket>(self.handle);

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

impl Drop for TcpSocket {
    fn drop(&mut self) {
        let mut inner = net::stack().inner.as_ref().unwrap().borrow_mut();
        inner.interface.remove_socket(self.handle);
    }
}

impl embedded_io::Io for TcpSocket {
    type Error = Error;
}

impl AsyncRead for TcpSocket {
    type ReadFuture<'a> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'a;

    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Self::ReadFuture<'a> {
        poll_fn(|cx| self.poll_read(cx, buf))
    }
}

impl AsyncWrite for TcpSocket {
    type WriteFuture<'a> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'a;

    fn write<'a>(&'a mut self, buf: &'a [u8]) -> Self::WriteFuture<'a> {
        poll_fn(|cx| self.poll_write(cx, buf))
    }

    type FlushFuture<'a> = impl Future<Output = Result<(), Self::Error>>
    where
        Self: 'a;

    fn flush<'a>(&'_ mut self) -> Self::FlushFuture<'_> {
        poll_fn(|_| Poll::Ready(Ok(())))
    }
}
