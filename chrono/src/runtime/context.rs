use core::cell::RefCell;

use once_cell::unsync::OnceCell;

use super::runtime::Handle;
use super::runtime::Spawner;
use crate::io::reactor::Handle as IoHandle;

static CONTEXT: Context = Context::new();

#[derive(Clone)]
pub(crate) struct Context(RefCell<OnceCell<Handle>>);

// Since we are in a single-threaded environment, it is safe to implement
// this trait this even though the OnceCell we are using is not thread safe
unsafe impl Sync for Context {}

impl Context {
    pub(crate) const fn new() -> Context {
        Context(RefCell::new(OnceCell::new()))
    }

    pub(crate) fn set(&self, handle: Handle) {
        let _ = self.0.borrow().set(handle);
    }

    pub(crate) fn io(&self) -> IoHandle {
        let inner = self.0.borrow();
        let handle = inner.get().expect("No reactor running");
        handle.io.clone()
    }

    pub(crate) fn spawner(&self) -> Spawner {
        let inner = self.0.borrow();
        let handle = inner.get().expect("No reactor running");
        handle.spawner.clone()
    }

    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&RefCell<OnceCell<Handle>>) -> R,
    {
        f(&self.0)
    }
}

pub(crate) struct EnterGuard;

impl Drop for EnterGuard {
    fn drop(&mut self) {
        tracing::debug!("Dropping enter guard");
        CONTEXT.with(|ctx| {
            ctx.borrow_mut().take();
        })
    }
}

/// Sets this [`Handle`] as the current [`Handle`]. Returns an
/// [`EnterGuard`] which clears thread local storage once dropped
pub(super) fn enter(new: Handle) -> EnterGuard {
    CONTEXT.set(new);
    EnterGuard {}
}

// ===== Functions for retrieving handles =====

pub(crate) fn io() -> IoHandle {
    CONTEXT.io()
}

pub(crate) fn spawner() -> Spawner {
    CONTEXT.spawner()
}
