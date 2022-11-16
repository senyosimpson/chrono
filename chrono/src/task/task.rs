use core::cell::Cell;
use core::fmt::Display;
use core::ptr::NonNull;

use crate::runtime::queue::Batch;
use crate::time::instant::Instant;

use super::header::Header;

#[derive(Clone, Copy)]
pub struct Task {
    pub id: TaskId,
    pub raw: NonNull<()>,
    pub(crate) batch: Batch,
    pub(crate) tasks: Pointers,
    pub(crate) timers: Pointers,
}

#[derive(Clone, Copy, defmt::Format)]
pub struct TaskId(u64);

#[derive(Clone, Copy, Default)]
pub(crate) struct Pointers {
    next: Option<NonNull<Task>>,
    prev: Option<NonNull<Task>>,
}

// ===== impl Task =====

impl Task {
    /// Create a new task
    pub fn new(ptr: NonNull<()>) -> Task {
        Task {
            id: TaskId::new(),
            raw: ptr,
            batch: Batch(1),
            tasks: Pointers::default(),
            timers: Pointers::default(),
        }
    }

    pub fn as_ptr(&self) -> NonNull<Task> {
        unsafe { NonNull::new_unchecked(self as *const _ as *mut _) }
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
        match header.expiry {
            Some(expiry) => now > expiry,
            None => false,
        }
    }

    /// If a timer has been set, the instant in time it will expire
    pub(crate) fn expiry(&self) -> Option<Instant> {
        let ptr = self.raw.as_ptr();
        let header = unsafe { &*(ptr as *const Header) };
        header.expiry
    }

    /// Clears expiry
    pub(crate) fn clear_expiry(&self) {
        let ptr = self.raw.as_ptr();
        let header = unsafe { &mut *(ptr as *mut Header) };
        header.expiry = None;
    }

    pub fn set_batch(&mut self, batch: Batch) {
        self.batch = batch
    }

    pub fn batch(&self) -> Batch {
        self.batch
    }
}

// ===== impl Pointers =====

impl Pointers {
    pub fn next(&self) -> Option<NonNull<Task>> {
        self.next
    }

    pub fn prev(&self) -> Option<NonNull<Task>> {
        self.prev
    }

    pub(crate) fn set_next(&mut self, task: Option<NonNull<Task>>) {
        self.next = task;
    }

    #[allow(dead_code)]
    pub(crate) fn set_prev(&mut self, task: Option<NonNull<Task>>) {
        self.prev = task;
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

// Safe since we are in a single-threaded environment
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
