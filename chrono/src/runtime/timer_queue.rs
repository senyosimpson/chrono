use core::cell::Cell;
use core::ptr::NonNull;

use crate::task::Task;
use crate::time::Instant;

pub(crate) struct TimerQueue {
    pub head: Cell<Option<NonNull<Task>>>,
    pub tail: Cell<Option<NonNull<Task>>>,
    deadline: Cell<Option<Instant>>,
}

// ===== impl TimerQueue =====

impl TimerQueue {
    pub const fn new() -> TimerQueue {
        TimerQueue {
            head: Cell::new(None),
            tail: Cell::new(None),
            deadline: Cell::new(None),
        }
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline.get()
    }

    /// Add an element to the back of list
    pub fn push_back(&mut self, mut task: NonNull<Task>) {
        unsafe {
            if let Some(mut tail) = self.tail.get() {
                task.as_mut().timers.set_prev(Some(tail));

                tail.as_mut().timers.set_next(Some(task));
                self.tail.replace(Some(task));
                return;
            }

            self.head.replace(Some(task));
            self.tail.replace(Some(task));
        }
    }

    /// Process all timers in the timer queue. If a timer has expired, the
    /// task will be scheduled onto the runtime.
    /// We also take this opportunity to update the deadline, setting it to
    /// the shortest remaining time of all the timers in the queue
    pub fn process(&self, now: Instant) {
        let mut deadline = Instant::max();

        let mut curr = match self.head.get() {
            None => {
                defmt::warn!("NO HEAD");
                return;
            },
            Some(mut curr) => unsafe { curr.as_mut() },
        };

        loop {
            if curr.is_timer_complete(now) {
                defmt::debug!("{}, {}: Timer complete", curr.id, curr.generation);
                // Timer complete so we're going to remove this entry.
                curr.clear_expiry();

                // If the prev and next entry is null, we are the only element
                // in the queue
                if curr.timers.prev().is_none() && curr.timers.next().is_none() {
                defmt::debug!("{}, {}: Only element", curr.id, curr.generation);
                    // Set head and tail to None, nothing more to process
                    self.head.replace(None);
                    self.tail.replace(None);
                    // Schedule the task associated with the timer
                    curr.schedule();
                    break;
                }

                // If the next entry is null, we are the tail
                if curr.timers.next().is_none() {
                    // Update the previous timer to have no next pointer
                    let mut prev = curr.timers.prev().unwrap();
                    unsafe { prev.as_mut().timers.set_next(None); }

                    // Set the tail to the previous timer
                    self.tail.replace(curr.timers.prev());
                    // Clear the prev timer
                    curr.timers.set_prev(None);
                    // Schedule the task associated with the timer
                    curr.schedule();
                    break;
                }

                // If the previous entry is null, we are the head
                if curr.timers.prev().is_none() {
                    // Update the next timer to have no prev pointer
                    let mut next = curr.timers.next().unwrap();
                    unsafe { next.as_mut().timers.set_prev(None); }

                    // Move the head forward
                    self.head.replace(curr.timers.next());
                    // Clear the next timer
                    curr.timers.set_next(None);
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
