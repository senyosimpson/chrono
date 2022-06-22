use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

use crate::task::Task;
use crate::time::Instant;

#[derive(Clone, Copy)]
pub(crate) struct TaskQueue {
    list: LinkedList,
}

#[derive(Clone, Copy)]
pub(crate) struct TimerQueue {
    list: LinkedList,
    deadline: Option<Instant>,
}

#[derive(Clone, Copy)]
pub(crate) struct LinkedList {
    pub head: Option<NonNull<Task>>,
    pub tail: Option<NonNull<Task>>,
}

// ===== impl TaskQueue =====

impl TaskQueue {
    pub const fn new() -> TaskQueue {
        TaskQueue {
            list: LinkedList::new(),
        }
    }
}

impl Deref for TaskQueue {
    type Target = LinkedList;

    fn deref(&self) -> &Self::Target {
        &self.list
    }
}

impl DerefMut for TaskQueue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.list
    }
}

unsafe impl Sync for TaskQueue {}

// ===== impl TimerQueue =====

impl TimerQueue {
    pub const fn new() -> TimerQueue {
        TimerQueue {
            list: LinkedList::new(),
            deadline: None,
        }
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub fn process(&mut self, now: Instant) {
        let mut deadline = Instant::max();

        let mut curr = match self.head {
            None => return,
            Some(mut curr) => unsafe { curr.as_mut() },
        };

        loop {
            if curr.is_timer_complete(now) {
                // Timer complete so we're going to remove this entry.
                curr.clear_expiry();

                // If the next entry is null, we are the tail
                if curr.timers.next().is_none() {
                    // Set head and tail to None, nothing more to process
                    self.head = None;
                    self.tail = None;
                    // Schedule the task associated with the timer
                    curr.schedule();
                    break;
                }

                // If the previous entry is null, we are the head
                if curr.timers.prev().is_none() {
                    // Move the head forward
                    self.head = curr.timers.next();
                    // Schedule the task associated with the timer
                    curr.schedule();
                    // Set curr to the new head for the next loop
                    curr = unsafe { self.head.unwrap().as_mut() };
                    continue;
                }

                // We are some random element in the middle.
                unsafe {
                    // TODO: Better error handling
                    let mut next = curr.timers.next().unwrap();
                    let mut prev = curr.timers.next().unwrap();

                    // Since we are removing an element in the middle, we have
                    // to update references.
                    //   1. The current element's prev must update its next pointer
                    //      to the current element's next.
                    //   2. The current element's next must update its prev pointer
                    //      to the current element's prev.
                    next.as_mut().timers.set_prev(Some(prev));
                    prev.as_mut().timers.set_next(Some(next));

                    // Set the next and prev to None
                    curr.timers.set_next(None);
                    curr.timers.set_next(None);

                    // Schedule the task associated with the timer
                    curr.schedule();
                    // Set curr to the next task in the list for the next loop
                    curr = next.as_mut();
                    continue;
                }
            }

            // The timer is not finished. Check to see if it should become the new deadline
            if let Some(t) = curr.expiry() {
                if t < deadline {
                    defmt::debug!("Setting deadline");
                    deadline = t
                }
            }

            // We are the tail, so we're just going to continue with our day
            if curr.timers.next().is_none() {
                break;
            }

            // Continue through the list
            curr = unsafe { curr.timers.next().unwrap().as_mut() };
        }

        if deadline != Instant::max() {
            self.deadline = Some(deadline)
        } else {
            self.deadline = None
        }
    }
}

impl Deref for TimerQueue {
    type Target = LinkedList;

    fn deref(&self) -> &Self::Target {
        &self.list
    }
}

impl DerefMut for TimerQueue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.list
    }
}

unsafe impl Sync for TimerQueue {}

// ===== impl LinkedList =====

impl LinkedList {
    pub const fn new() -> LinkedList {
        LinkedList {
            head: None,
            tail: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    pub fn push_back(&mut self, task: NonNull<Task>) {
        defmt::debug!("Inserting into task queue");
        unsafe {
            if let Some(mut tail) = self.tail {
                tail.as_mut().tasks.set_next(Some(task));
                self.tail = Some(task);
                return;
            }

            self.head = Some(task);
            self.tail = Some(task);
        }
    }

    pub fn pop(&mut self) -> Option<&mut Task> {
        match self.head {
            None => None,
            Some(mut head) => {
                let curr = unsafe { head.as_mut() };
                if curr.tasks.next().is_none() {
                    // We are on the last element in the queue. Set
                    // head and tail to None and return the task
                    self.head = None;
                    self.tail = None;
                    return Some(curr);
                }

                // We need to update references
                // Set the head to the next timer the current
                // head is pointing to
                self.head = curr.tasks.next();
                // Set next timer in the current task to null
                curr.tasks.set_next(None);
                // Return the current task
                Some(curr)
            }
        }
    }
}
