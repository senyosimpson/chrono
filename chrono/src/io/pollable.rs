use core::task::{Context, Poll};

use std::io::{self, Read, Write};
use std::os::unix::prelude::AsRawFd;
use std::rc::Rc;

use futures::ready;

use super::epoll::Interest;
use super::io_source::{Direction, IoSource};
use super::reactor::Handle;

/// Bridges the event queue and IO resources
pub(crate) struct Pollable<T> {
    /// The IO resource
    io: T,
    /// Stores
    source: Rc<IoSource>,
    /// Handle to the reactor
    handle: Handle,
}

impl<T> Pollable<T> {
    pub fn get_ref(&self) -> &T {
        &self.io
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.io
    }
}

// impl<T> Unpin for Pollable<T> {}

impl<T> Pollable<T> {
    pub fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.source.poll_readable(cx)
    }

    pub fn poll_writable(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.source.poll_writable(cx)
    }
}

impl<T: AsRawFd> Pollable<T> {
    pub fn new(io: T) -> io::Result<Self> {
        let interest = Interest::READABLE | Interest::WRITABLE;
        Self::new_with_interest(io, interest)
    }

    pub fn new_with_interest(io: T, interest: Interest) -> io::Result<Self> {
        let handle = Handle::current();
        let source = handle.inner.register(io.as_raw_fd(), interest)?;
        Ok(Pollable { io, source, handle })
    }
}

impl<T: Read> Pollable<T> {
    pub fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        loop {
            ready!(self.poll_readable(cx))?;

            match self.get_mut().read(buf) {
                Ok(n) => return Poll::Ready(Ok(n)),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Clear readiness for the specific direction
                    self.source.clear_readiness(Direction::Read)
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }
    }
}

impl<T: Write> Pollable<T> {
    pub fn poll_write(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        loop {
            ready!(self.poll_writable(cx))?;

            match self.get_mut().write(buf) {
                Ok(n) => return Poll::Ready(Ok(n)),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Clear readiness for the specific direction
                    self.source.clear_readiness(Direction::Write)
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }
    }
}

impl<T> Drop for Pollable<T> {
    fn drop(&mut self) {
        let inner = self.handle.inner();
        let _ = inner.deregister(self.source.token);
    }
}
