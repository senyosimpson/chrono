// Multiple sockets can listen on same port (this is how we create a backlog)

use core::cell::UnsafeCell;
use core::future::{poll_fn, Future};
use core::task::{Context, Poll};

use smoltcp::iface::{Interface, SocketHandle};
use smoltcp::socket::{TcpSocket, TcpSocketBuffer, TcpState};
use smoltcp::wire::IpEndpoint;

use super::devices::Enc28j60;
use crate::io::{AsyncRead, AsyncWrite};

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

pub struct TcpListener<'a> {
    /// The network interface for the ethernet driver
    interface: &'a UnsafeCell<Interface<'static, Enc28j60>>,
    /// The handle to the TCP socket
    handle: SocketHandle,
}

// ===== impl TcpListener =====

impl<'a> TcpListener<'a> {
    pub fn new(
        interface: &'a UnsafeCell<Interface<'static, Enc28j60>>,
        rx_buffer: &'static mut [u8],
        tx_buffer: &'static mut [u8],
    ) -> TcpListener<'a> {
        // Why does this work?
        // let rx_buffer: &'static mut [u8] = unsafe { core::mem::transmute(rx_buffer) };
        // let tx_buffer: &'static mut [u8] = unsafe { core::mem::transmute(tx_buffer) };

        let tcp_rx_buffer = TcpSocketBuffer::new(&mut rx_buffer[..]);
        let tcp_tx_buffer = TcpSocketBuffer::new(&mut tx_buffer[..]);
        let socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);

        let handle = unsafe { (*interface.get()).add_socket(socket) };

        TcpListener { interface, handle }
    }

    pub fn bind<T: Into<IpEndpoint>>(&mut self, addr: T) -> Result<(), Error> {
        // TODO: Make N listeners for a given endpoint

        let socket = unsafe { (*self.interface.get()).get_socket::<TcpSocket>(self.handle) };

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

    pub async fn accept(&self) -> Result<(TcpStream<'a>, IpEndpoint), Error> {
        let socket = unsafe { (*self.interface.get()).get_socket::<TcpSocket>(self.handle) };

        poll_fn(|cx| match socket.state() {
            TcpState::Listen | TcpState::SynReceived | TcpState::SynSent => {
                socket.register_send_waker(cx.waker()); // What wakes this up? Smoltcp might have inbuilt functionality for this
                Poll::Pending
            }
            _ => Poll::Ready(())
        })
        .await;

        let tcp_stream = TcpStream {
            interface: self.interface,
            handle: self.handle,
        };

        Ok((tcp_stream, socket.remote_endpoint()))
    }
}

pub struct TcpStream<'a> {
    /// The network interface for the ethernet driver
    interface: &'a UnsafeCell<Interface<'static, Enc28j60>>,
    /// Handle to a TCP socket
    handle: SocketHandle,
}

// ===== impl TcpStream =====

impl<'a> TcpStream<'a> {
    pub fn new(
        interface: &'a UnsafeCell<Interface<'static, Enc28j60>>,
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
