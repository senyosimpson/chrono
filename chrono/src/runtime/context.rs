use std::cell::RefCell;

use super::runtime::Handle;
use super::runtime::Spawner;
use crate::io::reactor::Handle as IoHandle;

thread_local! {
    static CONTEXT: RefCell<Option<Handle>> = RefCell::new(None)
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
    match CONTEXT.try_with(|ctx| {
        ctx.borrow_mut().replace(new);
        EnterGuard {}
    }) {
        Ok(enter_guard) => enter_guard,
        Err(_) => panic!("Thread local destroyed"),
    }
}

// ===== Functions for retrieving handles =====

pub(crate) fn io() -> IoHandle {
    match CONTEXT.try_with(|ctx| {
        let ctx = ctx.borrow();
        let handle = ctx.as_ref().expect("No reactor running");
        handle.io.clone()
    }) {
        Ok(io_handle) => io_handle,
        Err(_) => panic!("Thread local destroyed"),
    }
}

pub(crate) fn spawner() -> Spawner {
    match CONTEXT.try_with(|ctx| {
        let ctx = ctx.borrow();
        ctx.as_ref()
            .map(|handle| handle.spawner.clone())
            .expect("No reactor running")
    }) {
        Ok(spawner) => spawner,
        Err(_) => panic!("Thread local destroyed"),
    }
}
