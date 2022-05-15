use core::ptr;

use crate::task::Task;

#[derive(Debug, Clone, Copy)]
pub struct Queue {
    pub head: *mut Task,
    pub tail: *mut Task,
}

unsafe impl Sync for Queue {}

impl Queue {
    pub const fn new() -> Queue {
        Queue {
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    pub fn push_back(&mut self, task: *mut Task) {
        defmt::debug!("Inserting into list: Task {}", task);
        defmt::debug!("Head ptr: {}", self.head);
        defmt::debug!("Tail ptr: {}", self.tail);

        if self.head.is_null() {
            defmt::debug!("Head is null. Setting node to head and tail");
            self.head = task;
            self.tail = task;
        } else {
            defmt::debug!("Head exists. Setting next in current tail to the new task and the new task as the tail");
            unsafe { (*self.tail).next = task };
            self.tail = task;
        }

        defmt::debug!("Inserted into list");
        defmt::debug!("Head ptr: {}", self.head);
        defmt::debug!("Tail ptr: {}", self.tail);
    }

    pub fn pop(&mut self) -> Option<&Task> {
        defmt::debug!("Popping from list");
        defmt::debug!("Head ptr: {}", self.head);
        defmt::debug!("Tail ptr: {}", self.tail);
        // If self.head is None, it means we don't have anything
        // in the queue
        if self.head.is_null() {
            return None;
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
