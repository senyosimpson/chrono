use core::cell::Cell;

use super::runtime::Handle;
use super::runtime::Spawner;
use crate::time::driver::{self, Driver};

static CONTEXT: Context = Context::new();

#[derive(Clone)]
pub(crate) struct Context(Cell<Option<Handle>>);

// Since we are in a single-threaded environment, it is safe to implement
// this trait this even though the Cell we are using is not thread safe
unsafe impl Sync for Context {}

impl Context {
    const fn new() -> Context {
        Context(Cell::new(None))
    }

    fn spawner(&self) -> Spawner {
        let inner = self.0.get();
        let handle = inner.as_ref().expect("No reactor running");
        handle.spawner
    }

    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Cell<Option<Handle>>) -> R,
    {
        f(&self.0)
    }
}

pub(crate) struct EnterGuard;

impl Drop for EnterGuard {
    fn drop(&mut self) {
        defmt::debug!("Dropping enter guard");
        CONTEXT.with(|ctx| {
            ctx.get().take();
        })
    }
}

/// Sets this [`Handle`] as the current [`Handle`]. Returns an
/// [`EnterGuard`] which clears thread local storage once dropped
pub(super) fn enter(new: Handle) -> EnterGuard {
    CONTEXT.with(|ctx| ctx.replace(Some(new)));
    EnterGuard {}
}

// ===== Functions for retrieving handles =====

pub(crate) fn spawner() -> Spawner {
    CONTEXT.spawner()
}

pub(crate) fn time_driver() -> &'static mut Driver {
    driver::driver()
}