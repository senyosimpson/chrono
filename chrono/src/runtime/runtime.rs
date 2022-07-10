use core::future::Future;
use core::marker::PhantomData;
use core::ptr::NonNull;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use super::context;
use super::queue::{TaskQueue, TimerQueue};
use crate::task::join::JoinHandle;
use crate::task::RawTask;
use crate::time::instant::Instant;

pub struct Runtime {
    /// Queue of tasks
    pub(crate) tasks: TaskQueue,
    /// Queue of timers
    pub(crate) timers: TimerQueue,
}

/// Handle to the runtime
#[derive(Clone, Copy)]
pub struct Handle {
    /// Spawner responsible for spawning tasks onto the executor
    pub(crate) spawner: Spawner,
}

/// Spawns tasks onto the executor
#[derive(Clone, Copy)]
pub struct Spawner {
    rt: &'static Runtime,
}

// ===== impl Runtime =====

impl Runtime {
    pub const fn new() -> Runtime {
        let tasks = TaskQueue::new();
        let timers = TimerQueue::new();

        Runtime { tasks, timers }
    }

    /// Get the handle to the runtime
    pub fn handle(&'static self) -> Handle {
        Handle {
            spawner: Spawner { rt: self },
        }
    }

    /// Spawn a task onto the runtime
    pub fn spawn<F: Future<Output = T>, T>(
        &'static self,
        raw: RawTask<F, T>,
    ) -> Result<JoinHandle<T>, SpawnError> {
        self.handle().spawn(raw)
    }

    pub fn block_on<F: Future>(&'static self, future: F) -> F::Output {
        // Enter runtime context
        let _enter = context::enter(self.handle());

        crate::pin!(future);

        let waker = unsafe { Waker::from_raw(NoopWaker::waker()) };
        let cx = &mut Context::from_waker(&waker);

        loop {
            // If the future is ready, return the output
            defmt::debug!("Polling `block_on` future");
            if let Poll::Ready(v) = future.as_mut().poll(cx) {
                defmt::debug!("`block_on` future ready");
                return v;
            }
            defmt::debug!("`block_on` future pending");

            // If the task queue is empty, wait for an event/interrupt
            if self.tasks.is_empty() {
                defmt::debug!("Queue empty. Waiting for event");
                cortex_m::asm::wfe()
            }

            // Process all timers
            let now = Instant::now();
            self.timers.process(now);

            // Start the timer if there is a deadline
            if let Some(deadline) = self.timers.deadline() {
                let dur = deadline - Instant::now();
                context::time_driver().start(dur);
                defmt::debug!("Started timer. Deadline in {}", dur);
            }

            loop {
                let task = self.tasks.pop_front();
                match task {
                    Some(task) => {
                        defmt::debug!("Task {}: Popped off executor queue and running", task.id);
                        task.run()
                    }
                    None => break,
                }
            }
        }
    }
}

// Safe since we are in a single-threaded environment
unsafe impl Sync for Runtime {}

// ===== impl Handle =====

impl Handle {
    pub fn spawn<F: Future<Output = T>, T>(
        &self,
        raw: RawTask<F, T>,
    ) -> Result<JoinHandle<T>, SpawnError> {
        self.spawner.spawn(raw)
    }
}

// ===== impl Spawner =====

pub enum SpawnError {
    QueueFull,
}

impl Spawner {
    pub fn spawn<F: Future<Output = T>, T>(
        &self,
        raw: RawTask<F, T>,
    ) -> Result<JoinHandle<T>, SpawnError> {
        let memory = raw.memory();

        let rt = unsafe { NonNull::new_unchecked(self.rt as *const _ as *mut _) };
        memory.rt.replace(rt);

        // pointer to Memory inside of RawTask
        let ptr = unsafe { NonNull::new_unchecked(raw.ptr) };

        let join_handle = JoinHandle {
            raw: ptr,
            _marker: PhantomData,
        };

        // Get a pointer to our task to store in the queue
        let task = memory.task();
        task.schedule();

        let spawned: Result<(), ()> = Ok(());
        if spawned.is_err() {
            return Err(SpawnError::QueueFull);
        }
        defmt::debug!("Task {}: Spawned", task.id);

        Ok(join_handle)
    }
}

// ===== No op waker =====

struct NoopWaker;

impl NoopWaker {
    fn waker() -> RawWaker {
        fn no_op(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            NoopWaker::waker()
        }

        let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
        RawWaker::new(0 as *const (), vtable)
    }
}
