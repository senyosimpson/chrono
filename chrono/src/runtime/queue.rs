use core::ptr;

use crate::task::Task;

pub struct Queue {
    head: *mut Task,
    tail: *mut Task,
}

impl Queue {
    pub fn new() -> Queue {
        Queue {
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
        }
    }

    pub fn insert(&mut self, task: *mut Task) {
        if self.head.is_null() {
            self.head = task;
            self.tail = task;
        } else {
            unsafe { (*self.tail).next = task };
            self.tail = task;
        }
    }

    pub fn pop(&mut self) -> Option<&Task> {
        // If self.head is None, it means we don't have anything
        // in the queue
        if self.head.is_null() {
            return None
        }

        // If we are on the last element in the queue, head and tail will be the same.
        // We need to set both head and tail to null
        // If we still have more elements, we move head to the next element
        if self.head == self.tail {
            let task = unsafe { &mut *self.head };
            self.head = ptr::null_mut();
            self.tail = ptr::null_mut();
            Some(task)
        } else {
            let task = unsafe { &mut *self.head };
            self.head = task.next;
            task.next = ptr::null_mut();
            Some(task)
        }
    }
}