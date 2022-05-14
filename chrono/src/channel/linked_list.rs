use core::ptr;

use super::semaphore::Waiter;

pub(crate) struct LinkedList {
    head: *mut Waiter,
    tail: *mut Waiter,
}

impl LinkedList {
    pub const fn new() -> LinkedList {
        LinkedList {
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
        }
    }

    pub fn push_front(&mut self, value: *mut Waiter) {
        unsafe {
            (*value).next = self.head;
        }
        self.head = value;
    }

    pub fn push_back(&mut self, waiter: *mut Waiter) {
        if self.head.is_null() {
            self.head = waiter;
            self.tail = waiter;
        } else {
            unsafe {
                (*self.tail).next = waiter;
            }
            self.tail = waiter
        }
    }

    pub fn pop_front(&mut self) -> Option<&Waiter> {
        if self.head.is_null() {
            return None;
        }

        if self.head == self.tail {
            let waiter = unsafe { &mut *self.head };
            self.head = ptr::null_mut();
            self.tail = ptr::null_mut();
            Some(waiter)
        } else {
            let waiter = unsafe { &mut *self.head };
            self.head = waiter.next;
            waiter.next = ptr::null_mut();
            Some(waiter)
        }
    }

    pub fn pop_back(&mut self) -> Option<&Waiter> {
        if self.tail.is_null() {
            return None;
        }

        if self.head == self.tail {
            let waiter = unsafe { &mut *self.head };
            self.head = ptr::null_mut();
            self.tail = ptr::null_mut();
            Some(waiter)
        } else {
            let waiter = unsafe { &mut *self.tail };
            self.tail = waiter.prev;
            Some(waiter)
        }

    }
}
