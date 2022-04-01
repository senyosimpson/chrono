use core::cell::RefCell;

use super::runtime::Handle;
use super::runtime::Spawner;
use crate::io::reactor::Handle as IoHandle;

static CONTEXT: Context = Context::new();

#[derive(Clone)]
pub(crate) struct Context(RefCell<Option<Handle>>);

// Since we are in a single-threaded environment, it is safe to implement
// this trait this even though the OnceCell we are using is not thread safe
unsafe impl Sync for Context {}

impl Context {
    const fn new() -> Context {
        Context(RefCell::new(None))
    }

    fn io(&self) -> IoHandle {
        let inner = self.0.borrow();
        let handle = inner.as_ref().expect("No reactor running");
        // let handle = inner.get().expect("No reactor running");
        handle.io.clone()
    }

    fn spawner(&self) -> Spawner {
        let inner = self.0.borrow();
        let handle = inner.as_ref().expect("No reactor running");
        handle.spawner.clone()
    }

    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&RefCell<Option<Handle>>) -> R,
    {
        f(&self.0)
    }
}

pub(crate) struct EnterGuard;

impl Drop for EnterGuard {
    fn drop(&mut self) {
        defmt::debug!("Dropping enter guard");
        CONTEXT.with(|ctx| {
            ctx.borrow_mut().take();
        })
    }
}

/// Sets this [`Handle`] as the current [`Handle`]. Returns an
/// [`EnterGuard`] which clears thread local storage once dropped
pub(super) fn enter(new: Handle) -> EnterGuard {
    CONTEXT.with(|ctx| ctx.borrow_mut().replace(new));
    EnterGuard {}
}

// ===== Functions for retrieving handles =====

pub(crate) fn io() -> IoHandle {
    CONTEXT.io()
}

pub(crate) fn spawner() -> Spawner {
    CONTEXT.spawner()
}
