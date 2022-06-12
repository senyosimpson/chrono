use core::task::Waker;

use crate::task::raw::TaskVTable;
use crate::task::state::State;
use crate::task::Task;
use crate::time::instant::Instant;

pub struct Header {
    pub task: Task,
    pub state: State,
    pub timer_expiry: Option<Instant>,
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
