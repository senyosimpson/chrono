use core::cell::Cell;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

use crate::task::Task;
use crate::time::Instant;

pub(crate) struct TaskQueue {
    list: LinkedList,
}

pub(crate) struct TimerQueue {
    list: LinkedList,
    deadline: Cell<Option<Instant>>,
}

pub(crate) struct LinkedList {
    pub head: Cell<Option<NonNull<Task>>>,
    pub tail: Cell<Option<NonNull<Task>>>,
    pub generation: Cell<Generation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation(pub u8);

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

// Safe since we are in a single-threaded environment
unsafe impl Sync for TaskQueue {}

// ===== impl TimerQueue =====

impl TimerQueue {
    pub const fn new() -> TimerQueue {
        TimerQueue {
            list: LinkedList::new(),
            deadline: Cell::new(None),
        }
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline.get()
    }

    /// Process all timers in the timer queue. If a timer has expired, the
    /// task will be scheduled onto the runtime.
    /// We also take this opportunity to update the deadline, setting it to
    /// the shortest remaining time of all the timers in the queue
    pub fn process(&self, now: Instant) {
        let mut deadline = Instant::max();

        let mut curr = match self.head.get() {
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
                    self.head.replace(None);
                    self.tail.replace(None);
                    // Schedule the task associated with the timer
                    curr.schedule();
                    break;
                }

                // If the previous entry is null, we are the head
                if curr.timers.prev().is_none() {
                    // Move the head forward
                    self.head.replace(curr.timers.next());
                    // Schedule the task associated with the timer
                    curr.schedule();
                    // Set curr to the new head for the next loop
                    curr = unsafe { self.head.get().unwrap().as_mut() };
                    continue;
                }

                // We are some random element in the middle.
                unsafe {
                    // Safe to unwrap because we've already checked we aren't the head or tail
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
                    defmt::trace!("Setting deadline");
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
            self.deadline.replace(Some(deadline));
        } else {
            self.deadline.replace(None);
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

// Safe since we are in a single-threaded environment
unsafe impl Sync for TimerQueue {}

// ===== impl LinkedList =====

impl LinkedList {
    pub const fn new() -> LinkedList {
        LinkedList {
            head: Cell::new(None),
            tail: Cell::new(None),
            generation: Cell::new(Generation(0)),
        }
    }

    pub fn prepare(&self) {
        self.generation.replace(self.generation().next());
    }

    /// Get the current generation of the list
    pub fn generation(&self) -> Generation {
        self.generation.get()
    }

    /// Is the list empty?
    pub fn is_empty(&self) -> bool {
        self.head.get().is_none()
    }

    /// Add an element to the back of list
    pub fn push_back(&mut self, task: NonNull<Task>) {
        defmt::trace!("Inserting into task queue");
        unsafe {
            if let Some(mut tail) = self.tail.get() {
                tail.as_mut().tasks.set_next(Some(task));
                // Set the generation of the new task to the next generation
                // so that we only process it on the next round
                tail.as_mut()
                    .set_generation(self.generation().next());
                self.tail.replace(Some(task));
                return;
            }

            self.head.replace(Some(task));
            self.tail.replace(Some(task));
        }
    }

    /// Pop an item off the front of the list
    pub fn pop_front(&self) -> Option<&mut Task> {
        match self.head.get() {
            None => None,
            Some(mut head) => {
                let curr = unsafe { head.as_mut() };
                // If the task isn't from the current batch, return
                if curr.generation() != self.generation() {
                    return None;
                }

                if curr.tasks.next().is_none() {
                    // We are on the last element in the queue. Set
                    // head and tail to None and return the task
                    self.head.replace(None);
                    self.tail.replace(None);
                    return Some(curr);
                }

                // Set the head to the next timer the current head
                // is pointing to
                self.head.replace(curr.tasks.next());
                // Set next timer in the current task to null
                curr.tasks.set_next(None);
                // Return the current task
                Some(curr)
            }
        }
    }
}

impl Generation {
    pub fn next(&self) -> Generation {
        Generation(self.0.wrapping_add(1))
    }
}
