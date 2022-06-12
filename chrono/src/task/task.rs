use core::cell::Cell;
use core::fmt::Display;
use core::ptr::{self, NonNull};

use crate::time::instant::Instant;

use super::header::Header;

pub struct Task {
    pub id: TaskId,
    pub raw: NonNull<()>,
    pub(crate) tasks: Pointers,
    pub(crate) timers: Pointers,
}

#[derive(Clone, Copy, defmt::Format)]
pub struct TaskId(u64);

pub(crate) struct Pointers {
    next: *mut Task,
    prev: *mut Task,
}

// ===== impl Task =====

impl Task {
    /// Create a new task
    pub fn new(ptr: NonNull<()>) -> Task {
        Task {
            id: TaskId::new(),
            raw: ptr,
            tasks: Pointers::default(),
            timers: Pointers::default(),
        }
    }

    /// Run the task by calling its poll method
    pub fn run(&self) {
        let ptr = self.raw.as_ptr();
        let header = ptr as *const Header;
        unsafe { ((*header).vtable.poll)(ptr) }
    }

    pub fn schedule(&self) {
        let ptr = self.raw.as_ptr();
        let header = ptr as *const Header;
        unsafe { ((*header).vtable.schedule)(ptr) }
    }

    pub fn is_timer_complete(&self, now: Instant) -> bool {
        let ptr = self.raw.as_ptr();
        let header = unsafe { &*(ptr as *const Header) };
        match header.timer_expiry {
            Some(expiry) => now > expiry,
            None => false,
        }
    }

    pub(crate) fn timer_duration(&self) -> Option<Instant> {
        let ptr = self.raw.as_ptr();
        let header = unsafe { &*(ptr as *const Header) };
        header.timer_expiry
    }

    pub(crate) fn next_task(&self) -> *mut Task {
        self.tasks.next
    }

    pub(crate) fn next_timer(&self) -> *mut Task {
        self.timers.next
    }

    pub(crate) fn prev_timer(&self) -> *mut Task {
        self.timers.prev
    }

    pub(crate) fn set_next_task(&mut self, task: *mut Task) {
        self.tasks.next = task;
    }

    #[allow(dead_code)]
    pub(crate) fn set_prev_task(&mut self, task: *mut Task) {
        self.tasks.prev = task;
    }

    pub(crate) fn set_next_timer(&mut self, task: *mut Task) {
        self.timers.next = task;
    }

    pub(crate) fn set_prev_timer(&mut self, task: *mut Task) {
        self.timers.prev = task;
    }
}

// ===== impl Pointers =====

impl Pointers {
    pub fn is_next_null(&self) -> bool {
        self.next.is_null()
    }

    pub fn is_prev_null(&self) -> bool {
        self.prev.is_null()
    }
}

impl Default for Pointers {
    fn default() -> Self {
        Self {
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
        }
    }
}

// ===== impl TaskId =====

impl TaskId {
    pub fn new() -> Self {
        static ID: Counter = Counter::new();
        TaskId(ID.incr())
    }
}

impl Display for TaskId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A monotonic counter that is updated through interior
/// mutability. Allows it to be used as a static while still
/// being able to be updated
#[derive(Default)]
struct Counter(Cell<u64>);

// Implement sync for counter to enable it to be used as
// a static. It is safe to do so because we aren't sharing
// it across threads
unsafe impl Sync for Counter {}

impl Counter {
    const fn new() -> Counter {
        Counter(Cell::new(0))
    }

    pub fn incr(&self) -> u64 {
        let prev = self.0.get();
        let new = prev + 1;
        self.0.set(new);
        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn increment_counter() {
        let counter = Counter::default();
        assert_eq!(1, counter.incr());
        assert_eq!(2, counter.incr());
        assert_eq!(3, counter.incr());
    }
}
