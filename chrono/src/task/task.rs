use core::cell::Cell;
use core::fmt::Display;
use core::ptr::{self, NonNull};

use super::header::Header;

pub struct Task {
    pub id: TaskId,
    pub raw: NonNull<()>,
    pub next: *mut Task,
}

impl Task {
    pub fn new(ptr: NonNull<()>) -> Task {
        Task {
            id: TaskId::new(),
            raw: ptr,
            next: ptr::null_mut(),
        }
    }

    pub fn set(&mut self, ptr: NonNull<()>) {
        self.raw = ptr;
    }

    pub fn run(&self) {
        let ptr = self.raw.as_ptr();
        let header = ptr as *const Header;
        unsafe { ((*header).vtable.poll)(ptr) }
    }
}

/// A monotonic counter that is updated through interior
/// mutability. Allows it used as a static while still
/// being able to be updated
#[derive(Default)]
struct Counter(Cell<u64>);

#[derive(Clone, Copy, defmt::Format)]
pub struct TaskId(u64);

// ===== impl Counter =====

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
