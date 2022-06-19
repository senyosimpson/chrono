use core::ptr;

use crate::task::Task;
use crate::time::instant::Instant;

#[derive(Clone, Copy)]
pub struct Queue {
    pub head: *mut Task,
    pub tail: *mut Task,
}

// Safe since we are in a single-threaded environment
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
        defmt::debug!("Inserting into timer queue");
        if self.head.is_null() {
            self.head = task;
            self.tail = task;
        } else {
            unsafe { (*self.tail).set_next_timer(task) };
            self.tail = task;
        }
    }

    pub fn pop(&mut self) -> Option<&mut Task> {
        // If head is null, it means we don't have anything in the queue
        if self.head.is_null() {
            return None;
        }

        // If we are on the last element in the queue, head and tail will be the same.
        // We need to set both head and tail to null. If we still have more elements,
        // we move head to the next element
        if self.head == self.tail {
            // Get the head which will become the previous head
            let prev_head = unsafe { &mut *self.head };
            // Set the head and tail to null since we have no elements in our list
            self.head = ptr::null_mut();
            self.tail = ptr::null_mut();
            // Return the previous head
            Some(prev_head)
        } else {
            // Get the head which will become the previous head
            let prev_head = unsafe { &mut *self.head };
            // Set head of the list to the next timer the previous head was pointing to
            self.head = prev_head.next_timer();
            // Set next in the previous head to null
            prev_head.set_next_timer(ptr::null_mut());
            // Return the previous head
            Some(prev_head)
        }
    }

    pub fn process(&mut self, now: Instant) -> Option<Instant> {
        let mut deadline = Instant::max();

        if self.head.is_null() {
            return None;
        }

        let mut curr = unsafe { &mut *self.head };
        loop {
            if curr.is_timer_complete(now) {
                // Timer complete so we're going to remove this entry.

                // If the next entry is null, we are the tail. Set head and tail
                // to null and break. Nothing more to process
                if curr.timers.is_next_null() {
                    self.head = ptr::null_mut();
                    self.tail = ptr::null_mut();
                    // Schedule the task associated with the timer
                    curr.schedule();
                    break;
                }

                // If the previous entry is null, we are the head. Move the head
                // forward
                if curr.timers.is_prev_null() {
                    self.head = curr.next_timer();
                    // Schedule the task associated with the timer
                    curr.schedule();
                    // Set curr to the new head
                    curr = unsafe { &mut *self.head };
                }

                // Otherwise we are some random element in the middle. We need to perform
                // some gymnastics
                unsafe {
                    let next = &mut *curr.next_timer();
                    let prev = &mut *curr.prev_timer();

                    next.set_prev_timer(prev);
                    prev.set_next_timer(next);

                    curr.set_next_timer(ptr::null_mut());
                    curr.set_prev_timer(ptr::null_mut());

                    // Schedule the task associated with the timer
                    curr.schedule();
                    // Set curr to the next task in the list
                    curr = next;
                }
            } else {
                // It's not finished so we want to check if it should become the new deadline
                // TODO: Rename timer_duration
                if let Some(t) = curr.timer_duration() {
                    if t < deadline {
                        defmt::debug!("Setting deadline");
                        deadline = t
                    }
                }

                // We are the tail, so we're just going to continue with our day
                if curr.timers.is_next_null() {
                    break;
                }

                // Continue through the list
                curr = unsafe { &mut *curr.next_timer() };
            }
        }

        Some(deadline)
    }
}
