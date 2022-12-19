use core::{
    ops::Deref,
    task::{RawWaker, RawWakerVTable, Waker},
};

use super::header::Header;

/// A waker that does absolutely nothing
struct NoopWaker(RawWaker);

// ===== impl Noopwaker =====

impl NoopWaker {
    const RAW_WAKER_VTABLE: RawWakerVTable =
        RawWakerVTable::new(Self::clone, Self::no_op, Self::no_op, Self::no_op);

    pub fn new() -> NoopWaker {
        fn no_op(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            NoopWaker::new().into()
        }

        NoopWaker(RawWaker::new(0 as *const (), &Self::RAW_WAKER_VTABLE))
    }

    fn no_op(_: *const ()) {}

    fn clone(_: *const ()) -> RawWaker {
        NoopWaker::new().into()
    }
}

impl From<NoopWaker> for RawWaker {
    fn from(waker: NoopWaker) -> Self {
        RawWaker::new(0 as *const (), &NoopWaker::RAW_WAKER_VTABLE)
    }
}

impl From<NoopWaker> for Waker {
    fn from(value: NoopWaker) -> Self {
        unsafe { Waker::from_raw(NoopWaker::new().into()) }
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
