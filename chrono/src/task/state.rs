use super::task::TaskId;

// The task has been scheduled onto the executor
const SCHEDULED: usize = 1 << 0;

// The task is currently being run
const RUNNING: usize = 1 << 1;

// The task is complete
const COMPLETE: usize = 1 << 2;

// The join handle for the task still exists
const JOIN_HANDLE: usize = 1 << 3;

// The waker belonging to the join handle is registered
const JOIN_WAKER: usize = 1 << 4;

// Initial state of a task
const INITIAL_STATE: usize = SCHEDULED | JOIN_HANDLE;

pub struct State {
    pub state: usize,
    task_id: Option<TaskId>,
}

impl State {
    #[allow(unused)]
    pub fn new() -> State {
        State {
            state: INITIAL_STATE,
            task_id: None,
        }
    }

    pub fn new_with_id(task_id: TaskId) -> State {
        State {
            state: INITIAL_STATE,
            task_id: Some(task_id),
        }
    }

    pub fn has_join_handle(&mut self) -> bool {
        self.state & JOIN_HANDLE == JOIN_HANDLE
    }

    pub fn unset_join_handle(&mut self) {
        self.state &= !JOIN_HANDLE;
    }

    pub fn set_join_waker(&mut self) {
        self.state |= JOIN_WAKER;
    }

    pub fn has_join_waker(&self) -> bool {
        self.state & JOIN_WAKER == JOIN_WAKER
    }

    pub fn is_complete(&self) -> bool {
        self.state & COMPLETE == COMPLETE
    }

    pub fn set_complete(&mut self) {
        self.state |= COMPLETE;
    }

    pub fn is_scheduled(&self) -> bool {
        self.state & SCHEDULED == SCHEDULED
    }

    pub fn set_scheduled(&mut self) {
        self.state |= SCHEDULED;
    }

    pub fn unset_scheduled(&mut self) {
        self.state &= !SCHEDULED;
    }

    pub fn set_running(&mut self) {
        self.state |= RUNNING;
    }

    pub fn unset_running(&mut self) {
        self.state &= !RUNNING;
    }

    pub fn transition_to_complete(&mut self) {
        self.set_complete();
        self.unset_running();
        if let Some(task_id) = self.task_id {
            defmt::trace!(
                "Task {}: Transitioned to complete. State: {}",
                task_id,
                self
            );
        }
    }

    pub fn transition_to_running(&mut self) {
        self.set_running();
        self.unset_scheduled();
        if let Some(task_id) = self.task_id {
            defmt::trace!("Task {}: Transitioned to running. State: {}", task_id, self);
        }
    }

    pub fn transition_to_idle(&mut self) {
        self.unset_running();
        self.unset_scheduled();
        if let Some(task_id) = self.task_id {
            defmt::trace!("Task {}: Transitioned to idle. State: {}", task_id, self);
        }
    }

    pub fn transition_to_scheduled(&mut self) {
        self.set_scheduled();
        self.unset_running();
        if let Some(task_id) = self.task_id {
            defmt::trace!(
                "Task {}: Transitioned to scheduled. State: {}",
                task_id,
                self
            );
        }
    }
}

impl core::fmt::Display for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // scheduled | running | complete | join handle | join waker | ref count
        let scheduled = self.is_scheduled();
        let running = self.state & RUNNING == RUNNING;
        let complete = self.is_complete();
        let join_handle = self.state & JOIN_HANDLE == JOIN_HANDLE;
        let join_waker = self.has_join_waker();
        write!(
            f,
            "State {{ scheduled={}, running={}, complete={}, has_join_handle={}, has_join_waker={}}}",
            scheduled, running, complete, join_handle, join_waker
        )
    }
}

impl defmt::Format for State {
    fn format(&self, f: defmt::Formatter) {
        let scheduled = self.is_scheduled();
        let running = self.state & RUNNING == RUNNING;
        let complete = self.is_complete();
        let join_handle = self.state & JOIN_HANDLE == JOIN_HANDLE;
        let join_waker = self.has_join_waker();
        defmt::write!(
            f,
            "State {{ scheduled={}, running={}, complete={}, has_join_handle={}, has_join_waker={}}}",
            scheduled, running, complete, join_handle, join_waker
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_ref_count_ok() {
        let state = State::new();
        assert_eq!(state.ref_count(), 2);
    }

    #[test]
    fn incr_ref_count_ok() {
        let mut state = State::new();
        state.ref_incr();
        assert_eq!(state.ref_count(), 3);
    }

    #[test]
    fn decr_ref_count_ok() {
        let mut state = State::new();
        state.ref_decr();
        assert_eq!(state.ref_count(), 1);
    }
}
