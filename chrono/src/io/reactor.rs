use core::cell::RefCell;
use core::time::Duration;

use std::io;
use std::os::unix::prelude::RawFd;
use std::rc::Rc;

use slab::Slab;

use super::epoll::{Epoll, Events, Interest, Token};
use super::io_source::IoSource;

/// The reactor
///
/// It contains the event queue (epoll) and a list of all IO
/// resources in use. It is responsible for polling for new
/// events and dispatching them to the relevant handlers
pub(crate) struct Reactor {
    /// Collection of events. Used across calls to [`Epoll::poll`]
    events: Events,
    /// Shared state between the reactor and its handle
    inner: Rc<Inner>,
}

/// Handle to the reactor
#[derive(Clone)]
pub(crate) struct Handle {
    pub inner: Rc<Inner>,
}

pub(crate) struct Inner {
    /// The event queue
    pub poll: Epoll,
    /// Collection of IO resources registered in the event queue
    pub sources: RefCell<Slab<Rc<IoSource>>>,
}

impl Reactor {
    pub fn new() -> io::Result<Reactor> {
        Ok(Reactor {
            events: Events::with_capacity(1024),
            inner: Rc::new(Inner {
                poll: Epoll::new()?,
                sources: RefCell::new(Slab::new()),
            }),
        })
    }

    pub fn handle(&self) -> Handle {
        Handle {
            inner: self.inner.clone(),
        }
    }

    // Process new events
    pub fn react(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.inner.poll.poll(&mut self.events, timeout)?;

        for event in self.events.iter() {
            defmt::debug!(
                "Epoll: processing Event {{ token={}, interest={} }}",
                event.token().0,
                event.interest()
            );

            let token = event.token();
            if let Some(io_source) = self.inner.sources.borrow().get(token.0) {
                io_source.set_readiness(event);
                io_source.wake(event)
            }
        }

        Ok(())
    }
}

// ===== impl Handle =====

impl Handle {
    pub fn current() -> Self {
        crate::runtime::context::io()
    }

    pub fn inner(&self) -> Rc<Inner> {
        self.inner.clone()
    }
}

// ==== impl Inner =====

impl Inner {
    pub fn register(&self, io: RawFd, interest: Interest) -> io::Result<Rc<IoSource>> {
        defmt::debug!("Registering task in epoll");

        let mut sources = self.sources.borrow_mut();
        let entry = sources.vacant_entry();

        let token = Token(entry.key());
        let io_source = Rc::new(IoSource {
            io,
            token,
            ..Default::default()
        });

        self.poll.add(io, interest, token)?;

        entry.insert(io_source.clone());

        Ok(io_source)
    }

    pub fn deregister(&self, token: Token) -> io::Result<()> {
        defmt::debug!("Deregistering task from epoll");
        let source = self.sources.borrow_mut().remove(token.0);
        self.poll.delete(source.io)
    }
}
