use std::cell::Cell;
use std::fmt::Display;
use std::task::Waker;

use crate::task::raw::TaskVTable;
use crate::task::state::State;

pub(crate) struct Header {
    pub state: State,
    pub waker: Option<Waker>,        // Why is this wrapped in UnsafeCell?
    pub vtable: &'static TaskVTable, // Why &'static? Think cause they are fns
    pub id: TaskId,
}

impl Header {
    pub fn register_waker(&mut self, waker: &Waker) {
        self.waker = Some(waker.clone());
    }

    pub fn wake_join_handle(&self) {
        match &self.waker {
            Some(waker) => waker.wake_by_ref(),
            None => panic!("Missing waker!"),
        }
    }
}

/// A monotonic counter that is updated through interior
/// mutability. Allows it used as a static while still
/// being able to be updated
#[derive(Default)]
struct Counter(Cell<u64>);

#[derive(Clone, Copy)]
pub(crate) struct TaskId(u64);

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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
