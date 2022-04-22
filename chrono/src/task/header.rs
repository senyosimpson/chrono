use core::task::Waker;

use crate::task::Task;
use crate::task::raw::TaskVTable;
use crate::task::state::State;

pub struct Header {
    pub task: Task,
    pub state: State,
    pub waker: Option<Waker>,        // Why is this wrapped in UnsafeCell?
    pub vtable: &'static TaskVTable, // Why &'static? Think cause they are fns
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
