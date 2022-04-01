use core::cell::RefCell;
use core::task::{Context, Poll, Waker};

use std::io;
use std::os::unix::prelude::RawFd;

use super::epoll::{Event, Token};
use super::readiness::Readiness;

#[derive(Clone, Default)]
pub(crate) struct IoSource {
    /// Raw file descriptor of the IO resource
    pub(crate) io: RawFd,
    /// Token tying io source to slot in reactor slab
    pub(crate) token: Token,
    /// Holds state on an io resource's readiness for
    /// reading and writing
    pub(crate) inner: RefCell<Inner>,
}

#[derive(Clone, Default)]
pub(crate) struct Inner {
    /// Readiness of the source. Used to determine whether
    /// the source is ready for reading, writing or both
    pub(crate) readiness: Readiness,
    /// Waker registered by poll_readable
    pub(crate) reader: Option<Waker>,
    /// Waker registered by poll_writable
    pub(crate) writer: Option<Waker>,
}

#[derive(Clone, Copy)]
pub(crate) enum Direction {
    Read,
    Write,
}

impl IoSource {
    /// Set the readiness of the task (readable, writable or both)
    pub fn set_readiness(&self, event: &Event) {
        let mut inner = self.inner.borrow_mut();
        inner.readiness = Readiness::from_event(event)
    }

    /// Unset the bit indicating readiness for a specific [`Direction`]
    pub fn clear_readiness(&self, direction: Direction) {
        let mut inner = self.inner.borrow_mut();
        match direction {
            Direction::Read => inner.readiness = inner.readiness - Readiness::READABLE,
            Direction::Write => inner.readiness = inner.readiness - Readiness::WRITABLE,
        }
    }

    /// Wakes the task linked to this IO resource. Since reading and
    /// writing are separate tasks, it will inspect the event to
    /// determine if it is readable or writable or both and wake the
    /// relevant tasks.
    pub fn wake(&self, event: &Event) {
        let mut wakers = Vec::new();

        let mut inner = self.inner.borrow_mut();

        if event.is_readable() {
            if let Some(waker) = inner.reader.take() {
                wakers.push(waker)
            }
        }

        if event.is_writable() {
            if let Some(waker) = inner.writer.take() {
                wakers.push(waker)
            }
        }

        for waker in wakers {
            waker.wake()
        }
    }

    /// Determines whether the IO resource is ready to be polled for
    /// either reading or writing. In the event it is not ready, a
    /// waker is registered in the specified direction (read/write)
    pub(crate) fn poll_ready(
        &self,
        direction: Direction,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        match direction {
            Direction::Read => {
                if self.readable() {
                    return Poll::Ready(Ok(()));
                }
            }
            Direction::Write => {
                if self.writable() {
                    return Poll::Ready(Ok(()));
                }
            }
        }

        let mut inner = self.inner.borrow_mut();

        let slot = match direction {
            Direction::Read => &mut inner.reader,
            Direction::Write => &mut inner.writer,
        };

        match slot {
            Some(existing) => *existing = cx.waker().clone(),
            None => *slot = Some(cx.waker().clone()),
        }

        Poll::Pending
    }

    /// Determines whether the IO resource is ready for reading. Just
    /// forwards to [`poll_ready`] with a read [`Direction`]
    pub fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        defmt::debug!("Invoking poll_readable");
        let res = self.poll_ready(Direction::Read, cx);
        match res {
            Poll::Ready(Ok(())) => defmt::debug!("poll_readable returned Poll::Ready(ok)"),
            Poll::Ready(Err(_)) => defmt::debug!("poll_readable returned Poll::Ready(err)"),
            Poll::Pending => defmt::debug!("poll_readable returned Poll::Pending"),
        }
        res
    }

    /// Determines whether the IO resource is ready for reading. Just
    /// forwards to [`poll_ready`] with a write [`Direction`]
    pub fn poll_writable(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_ready(Direction::Write, cx)
    }

    /// Is the IO resource readable?
    pub fn readable(&self) -> bool {
        let inner = self.inner.borrow();
        inner.readiness & Readiness::READABLE == Readiness::READABLE
    }

    /// Is the IO resource writable?
    pub fn writable(&self) -> bool {
        let inner = self.inner.borrow();
        inner.readiness & Readiness::WRITABLE == Readiness::WRITABLE
    }
}
