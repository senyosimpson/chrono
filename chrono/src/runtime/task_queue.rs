use core::cell::Cell;
use core::ptr::NonNull;

use crate::task::Task;

pub(crate) struct TaskQueue {
    pub head: Cell<Option<NonNull<Task>>>,
    pub tail: Cell<Option<NonNull<Task>>>,
    pub generation: Cell<Generation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub struct Generation(pub u8);

// ===== impl TaskQueue =====

impl TaskQueue {
    pub const fn new() -> TaskQueue {
        TaskQueue {
            head: Cell::new(None),
            tail: Cell::new(None),
            generation: Cell::new(Generation(0)),
        }
    }

    pub fn prepare(&self) -> Generation {
        let generation = self.generation().next();
        self.generation.replace(generation);
        generation
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
    pub fn push_back(&mut self, mut task: NonNull<Task>) {
        unsafe {
            // Set the generation of the new task to the next generation
            // so that we only process it on the next round
            task.as_mut().set_generation(self.generation().next());

            if let Some(mut tail) = self.tail.get() {
                task.as_mut().tasks.set_prev(Some(tail));

                tail.as_mut().tasks.set_next(Some(task));
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

                    curr.tasks.set_next(None);
                    curr.tasks.set_prev(None);

                    return Some(curr);
                }

                let mut next = curr.tasks.next().unwrap();
                unsafe { next.as_mut().tasks.set_prev(None); }
                // Set the head to the next timer the current head
                // is pointing to
                self.head.replace(curr.tasks.next());
                // Set prev and next timer in the current task to null
                curr.tasks.set_next(None);
                curr.tasks.set_prev(None);

                // Return the current task
                Some(curr)
            }
        }
    }
}

// Safe since we are in a single-threaded environment
unsafe impl Sync for TaskQueue {}

// ===== impl Generation =====

impl Generation {
    pub fn next(&self) -> Generation {
        Generation(self.0.wrapping_add(1))
    }
}
