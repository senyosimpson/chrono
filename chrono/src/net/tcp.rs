// Multiple sockets can listen on same port (this is how we create a backlog)

use core::future::{poll_fn, Future};
use core::task::{Context, Poll};

use smoltcp::iface::SocketHandle;
use smoltcp::socket::{TcpSocket, TcpSocketBuffer, TcpState};
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

pub struct TcpListener {
    /// The handle to the TCP socket
    handle: SocketHandle,
}

// ===== impl TcpListener =====

impl TcpListener {
    pub fn new<'a>(rx_buffer: &'a mut [u8], tx_buffer: &'a mut [u8]) -> TcpListener {
        // Why does this work?
        let rx_buffer: &'static mut [u8] = unsafe { core::mem::transmute(rx_buffer) };
        let tx_buffer: &'static mut [u8] = unsafe { core::mem::transmute(tx_buffer) };

        let tcp_rx_buffer = TcpSocketBuffer::new(&mut rx_buffer[..]);
        let tcp_tx_buffer = TcpSocketBuffer::new(&mut tx_buffer[..]);
        let socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);

        let mut inner = net::stack().inner.as_ref().unwrap().borrow_mut();
        let handle = inner.interface.add_socket(socket);

        TcpListener { handle }
    }

    pub fn bind<T: Into<IpEndpoint>>(&mut self, addr: T) -> Result<(), Error> {
        // TODO: Make N listeners for a given endpoint

        let mut inner = net::stack().inner.as_ref().unwrap().borrow_mut();
        let socket = inner.interface.get_socket::<TcpSocket>(self.handle);

        match socket.listen(addr) {
            Ok(()) => {}
            Err(e) => match e {
                smoltcp::Error::Illegal => return Err(Error::AlreadyOpen),
                smoltcp::Error::Unaddressable => return Err(Error::InvalidPort),
                _ => unreachable!(),
            },
        }

        Ok(())
    }

    pub async fn accept(&self) -> Result<(TcpStream, IpEndpoint), Error> {
        let mut inner = net::stack().inner.as_ref().unwrap().borrow_mut();
        let socket = inner.interface.get_socket::<TcpSocket>(self.handle);

        poll_fn(|cx| match socket.state() {
            TcpState::Listen | TcpState::SynReceived | TcpState::SynSent => {
                socket.register_send_waker(cx.waker()); // What wakes this up? Smoltcp might have inbuilt functionality for this
                Poll::Pending
            }
            _ => Poll::Ready(()),
        })
        .await;

        let tcp_stream = TcpStream {
            handle: self.handle,
        };

        Ok((tcp_stream, socket.remote_endpoint()))
    }
}

pub struct TcpStream {
    /// Handle to a TCP socket
    handle: SocketHandle,
}

// ===== impl TcpStream =====

impl TcpStream {
    pub fn new(handle: SocketHandle) -> TcpStream {
        TcpStream { handle }
    }

    fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize, Error>> {
        // TODO: Sanity check grabbing this mutably
        let mut inner = net::stack().inner.as_ref().unwrap().borrow_mut();
        let socket = inner.interface.get_socket::<TcpSocket>(self.handle);

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

    fn poll_write(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>> {
        // TODO: Sanity check grabbing this mutably
        let mut inner = net::stack().inner.as_ref().unwrap().borrow_mut();
        let socket = inner.interface.get_socket::<TcpSocket>(self.handle);

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

impl embedded_io::Io for TcpStream {
    type Error = Error;
}

impl AsyncRead for TcpStream {
    type ReadFuture<'a> = impl Future<Output = Result<usize, Self::Error>>
    where
        Self: 'a;

    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Self::ReadFuture<'a> {
        poll_fn(|cx| self.poll_read(cx, buf))
    }
}

impl AsyncWrite for TcpStream {
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
