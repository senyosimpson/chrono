use core::task::{RawWaker, RawWakerVTable, Waker};

use super::header::Header;

/// A waker that does absolutely nothing
pub(crate) struct NoopWaker(RawWaker);

// ===== impl Noopwaker =====

impl NoopWaker {
    const RAW_WAKER_VTABLE: RawWakerVTable =
        RawWakerVTable::new(Self::clone, Self::no_op, Self::no_op, Self::no_op);

    pub fn raw() -> RawWaker {
        RawWaker::new(0 as *const (), &Self::RAW_WAKER_VTABLE)
    }

    fn no_op(_: *const ()) {}

    fn clone(_: *const ()) -> RawWaker {
        NoopWaker::raw()
    }
}

// ===== helpers =====

/// Get a pointer to a Waker's data
pub fn ptr(waker: &Waker) -> *const () {
    waker.as_raw().data()
}

/// Get the `header` of a Waker. Only applicable for Wakers' created
/// via a call to `RawTask::poll`
pub fn header(waker: &Waker) -> &Header {
    let raw_waker = waker.as_raw();
    let header = raw_waker.data() as *const Header;
    unsafe { &*header }
}
