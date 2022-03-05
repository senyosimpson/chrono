//! A safe library for interacting with epoll, specifically for this project.
//!
//! [`Epoll`] provides all the necessary functions to interact with epoll.
//!
//! This crate *does not* expose all the interest bitflags available for epoll
//! since they were not necessary for this project.

use std::fmt::Display;
use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Duration;

use bitflags::bitflags;
use libc;

/// Provides functionality for interacting epoll.
pub struct Epoll {
    pub fd: RawFd,
}

/// An equivalent of `libc::epoll_data`
///
/// Uses #[repr(C)] for interoperability with C.
/// Learn more [here](https://doc.rust-lang.org/reference/type-layout.html#the-c-representation)
///
/// Epoll events are packed, hence we must specify the packed configuration
/// as well. For more, read [here](https://doc.rust-lang.org/reference/type-layout.html#the-alignment-modifiers)
/// and [here](https://www.mikroe.com/blog/packed-structures-make-memory-feel-safe)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Event {
    interest: u32,
    data: u64,
}

/// A collection of [`Event`]s. This is passed into [`Epoll::poll`] which will
/// fill the buffer with ready events.
pub type Events = Vec<Event>;

bitflags! {
    pub struct Interest: u32 {
        const READABLE       = (libc::EPOLLET  | libc::EPOLLIN | libc::EPOLLRDHUP) as u32;
        const WRITABLE       = (libc::EPOLLET  | libc::EPOLLOUT) as u32;
    }
}

/// Control options for `epoll_ctl`
///
/// The enum representation is changed to i32 in order to work with
/// libc epoll bindings.
///
/// Learn more about type layout representations
/// [here](https://doc.rust-lang.org/reference/type-layout.html#representations)
#[repr(i32)]
pub(crate) enum CtlOp {
    /// Add an entry to the interest list
    Add = libc::EPOLL_CTL_ADD,
    /// Modify the interest of an associated entry in the interest list
    Mod = libc::EPOLL_CTL_MOD,
    /// Remove an entry from the interest list
    Del = libc::EPOLL_CTL_DEL,
}

/// Associates an entry in the interest list to an [`Event`]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Token(pub usize);

impl From<usize> for Token {
    fn from(value: usize) -> Token {
        Token(value)
    }
}

pub trait Source {
    fn raw_fd(&self) -> RawFd;
}

// ===== impl Source =====

impl Source for RawFd {
    fn raw_fd(&self) -> RawFd {
        *self
    }
}

impl<T: AsRawFd> Source for &T {
    fn raw_fd(&self) -> RawFd {
        self.as_raw_fd()
    }
}

// ==== impl Epoll =====

impl Epoll {
    pub fn new() -> io::Result<Epoll> {
        let fd = epoll::create()?;
        let poll = Epoll { fd };
        Ok(poll)
    }

    pub fn add(&self, source: impl Source, interest: Interest, token: Token) -> io::Result<()> {
        let event = Event::new(interest, token);
        epoll::ctl(self.fd, CtlOp::Add, source.raw_fd(), Some(event))?;
        Ok(())
    }

    pub fn delete(&self, source: impl Source) -> io::Result<()> {
        epoll::ctl(self.fd, CtlOp::Del, source.raw_fd(), None)?;
        Ok(())
    }

    #[allow(unused)]
    pub fn modify(&self, source: impl Source, interest: Interest, token: Token) -> io::Result<()> {
        let event = Event::new(interest, token);
        epoll::ctl(self.fd, CtlOp::Mod, source.raw_fd(), Some(event))?;
        Ok(())
    }

    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        events.clear();

        let timeout = match timeout {
            Some(duration) => duration.as_millis() as i32,
            None => -1, // TThis blocks indefinitely
        };
        let n_events = epoll::wait(self.fd, events, timeout)?;
        tracing::debug!("Epoll: Received {} events", n_events);

        // This is actually safe to call because `epoll::wait` returns the
        // number of events that were returned. Got this from Mio:
        // https://github.com/tokio-rs/mio/blob/22e885859bb481ae4c2827ab48552c3159fcc7f8/src/sys/unix/selector/epoll.rs#L77
        unsafe { events.set_len(n_events as usize) };
        Ok(())
    }

    pub fn close(&self) -> io::Result<()> {
        epoll::close(self.fd)
    }
}

impl Drop for Epoll {
    fn drop(&mut self) {
        tracing::debug!("Drop: epoll_fd={}", self.fd);
        let _ = self.close();
    }
}

// ===== impl Event =====

impl Event {
    pub fn new(interest: Interest, token: Token) -> Event {
        Event {
            interest: interest.bits(),
            data: token.0 as u64,
        }
    }

    pub fn token(&self) -> Token {
        Token(self.data as usize)
    }

    // EPOLLPRI means there is urgent data to read. It is possible for
    // there to be no standard data to read but *urgent* data to read hence we
    // also check for this flag
    //
    // EPOLLHUP means both halves of the connection have shutdown. When reading
    // from a socket, it indicates that the *peer* has closed it's writing half
    // of the socket (versus meaning we have closed our read half). There may still
    // be data to read in this case. We continue reading until we get a zero byte read
    //
    // EPOLLRDHUP means the that peer closed the connection (or its write half of the connection).
    // That means we've received all the data we need to and we should check if there is any
    // outstanding data to read in the socket
    pub fn is_readable(&self) -> bool {
        let interest = self.interest as libc::c_int;

        let epollin = interest & libc::EPOLLIN == libc::EPOLLIN;
        let epollpri = interest & libc::EPOLLPRI == libc::EPOLLPRI;
        let epollhup = interest & libc::EPOLLHUP == libc::EPOLLHUP;
        let epollrdhup = interest & libc::EPOLLRDHUP == libc::EPOLLRDHUP;

        // TODO: Needs santiy checking
        epollin || epollpri || epollhup || epollrdhup
    }

    pub fn is_writable(&self) -> bool {
        let interest = self.interest as libc::c_int;

        let epollout = interest & libc::EPOLLOUT == libc::EPOLLOUT;
        let epollhup = interest & libc::EPOLLHUP == libc::EPOLLHUP;

        epollout || epollhup
    }

    pub(crate) fn interest(&self) -> Interest {
        // Gauranteed to never panic so can unwrap
        Interest::from_bits(self.interest).unwrap()
    }
}

// ===== impl Interest =====

impl Display for Interest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let interest = self.bits() as libc::c_int;

        let epollin = interest & libc::EPOLLIN == libc::EPOLLIN;
        let epollpri = interest & libc::EPOLLPRI == libc::EPOLLPRI;
        let epollhup = interest & libc::EPOLLHUP == libc::EPOLLHUP;
        let epollrdhup = interest & libc::EPOLLRDHUP == libc::EPOLLRDHUP;
        let epollout = interest & libc::EPOLLOUT == libc::EPOLLOUT;

        write!(
            f,
            "Interest {{ epollin={}, epollout={}, epollpri={}, epollhup={}, epollrdhup={} }}",
            epollin, epollout, epollpri, epollhup, epollrdhup
        )
    }
}

// ===== Standalone functions wrapping libc::epoll_* calls =====

// For documentation of the various calls, refer to the
// [epoll man pages](https://man7.org/linux/man-pages/man7/epoll.7.html)
mod epoll {
    use super::{CtlOp, Event, Events};
    use std::io;
    use std::os::unix::prelude::RawFd;
    use std::ptr;

    // Safe wrapper around `libc::epoll_create1`
    // Sets the close-on-exec flag
    #[cfg(target_os = "linux")]
    pub(super) fn create() -> io::Result<RawFd> {
        cvt(unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) })
    }

    // Safe wrapper around `libc::epoll_ctl`
    // Event is None only in the case where we want to perform a delete
    // operation. epoll_ctl still expects a pointer so we pass in a null
    // pointer
    #[cfg(target_os = "linux")]
    pub(super) fn ctl(
        epfd: RawFd,
        op: CtlOp,
        fd: RawFd,
        mut event: Option<Event>,
    ) -> io::Result<()> {
        let event = match &mut event {
            Some(event) => event as *mut Event as *mut libc::epoll_event,
            None => ptr::null_mut(),
        };
        cvt(unsafe { libc::epoll_ctl(epfd, op as i32, fd, event) })?;
        Ok(())
    }

    // Safe wrapper around `libc::epoll_wait`
    #[cfg(target_os = "linux")]
    pub(super) fn wait(epfd: RawFd, events: &mut Events, timeout: i32) -> io::Result<i32> {
        let capacity = events.capacity() as i32;
        let events = events.as_mut_ptr() as *mut libc::epoll_event;
        cvt(unsafe { libc::epoll_wait(epfd, events, capacity, timeout) })
    }

    // Safe wrapper around `libc::close`
    #[cfg(target_os = "linux")]
    pub(super) fn close(fd: RawFd) -> io::Result<()> {
        cvt(unsafe { libc::close(fd) })?;
        Ok(())
    }

    // Converts C error codes into a Rust Result type
    fn cvt(result: i32) -> io::Result<i32> {
        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_epoll_instance() {
        // Test it works by creating an instance of epoll and then closing it
        // If this function does not work, it will panic
        let epoll = Epoll::new().unwrap();
        // Drop the error so we have a gaurantee that if the test fails, it is from
        // creating the epoll instance. This is arguably shady
        let _ = epoll.close();
    }

    #[test]
    fn add_event() {
        use std::net::TcpListener;
        use std::os::unix::io::AsRawFd;

        let epoll = Epoll::new().unwrap();
        let interest = Interest::READABLE | Interest::WRITABLE;
        let listener = TcpListener::bind("localhost:3000").unwrap();

        epoll.add(listener.as_raw_fd(), interest, Token(1)).unwrap();
        let _ = epoll.close();
    }

    #[test]
    fn poll_events() {
        use std::io::Write;
        use std::net::{TcpListener, TcpStream};
        use std::os::unix::io::AsRawFd;

        let epoll = Epoll::new().unwrap();
        let interest = Interest::READABLE;

        let listener = TcpListener::bind("localhost:3000").unwrap();
        epoll.add(listener.as_raw_fd(), interest, Token(1)).unwrap();

        let mut socket = TcpStream::connect("localhost:3000").unwrap();
        let request = "Hello world!";
        socket.write_all(request.as_bytes()).unwrap();

        let maxevents = 10;
        let mut events = Events::with_capacity(maxevents);
        epoll.poll(&mut events, None).unwrap();
        epoll.close().unwrap();

        assert_eq!(events.len(), 1);
    }
}
