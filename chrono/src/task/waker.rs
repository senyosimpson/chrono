use core::task::Waker;

use super::header::Header;

pub fn ptr(waker: &Waker) -> *const () {
    waker.as_raw().data()
}

pub fn header(waker: &Waker) -> &Header {
    let raw_waker = waker.as_raw();
    let header = raw_waker.data() as *const Header;
    unsafe { &*header }
}
