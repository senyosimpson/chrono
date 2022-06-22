use core::cell::RefCell;
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
    // Holds the task queue
    inner: RefCell<Inner>,
    // Handle to runtime
    handle: Handle,
}

struct Inner {
    /// Queue that holds tasks
    tasks: NonNull<TaskQueue>,
    /// Queue that holds timers
    timers: NonNull<TimerQueue>,
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
    /// Queue that holds tasks
    tasks: NonNull<TaskQueue>,
    /// Queue that holds timers
    timers: NonNull<TimerQueue>,
}

// ===== impl Runtime =====

impl Runtime {
    #[allow(non_upper_case_globals)]
    pub fn new() -> Runtime {
        let time_driver = context::time_driver();
        time_driver.init();

        static mut task_queue: TaskQueue = TaskQueue::new();
        static mut timer_queue: TimerQueue = TimerQueue::new();

        // Pointers to task and timer queues
        let tasks = unsafe { NonNull::new_unchecked(&task_queue as *const _ as *mut _) };
        let timers = unsafe { NonNull::new_unchecked(&timer_queue as *const _ as *mut _) };

        let inner = RefCell::new(Inner { tasks, timers });

        let spawner = Spawner { tasks, timers };
        let handle = Handle { spawner };

        Runtime { inner, handle }
    }

    // Get the handle to the runtime
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    // Spawn a task onto the runtime
    pub fn spawn<F: Future<Output = T>, T>(
        &self,
        raw: RawTask<F, T>,
    ) -> Result<JoinHandle<T>, SpawnError> {
        self.handle.spawn(raw)
    }

    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        // Enter runtime context
        let _enter = context::enter(self.handle);
        self.inner.borrow_mut().block_on(future)
    }
}

// ===== impl Inner =====

impl Inner {
    pub fn block_on<F: Future>(&mut self, future: F) -> F::Output {
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

            let task_queue = unsafe { self.tasks.as_mut() };
            if task_queue.is_empty() {
                defmt::debug!("Queue empty. Waiting for event");
                cortex_m::asm::wfe()
            }

            let timer_queue = unsafe { self.timers.as_mut() };
            let now = Instant::now();
            timer_queue.process(now);

            // Start the timer
            // NOTE: This will cause issues because initially, it will only start timing down
            // once the first batch of tasks have been processed
            if let Some(deadline) = timer_queue.deadline() {
                let dur = deadline - Instant::now();
                context::time_driver().start(dur);
                defmt::debug!("Started timer. Deadline in {}", dur);
            }

            loop {
                let task = task_queue.pop();
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
        // We need to write the scheduler into the RawTask
        memory.task_queue.replace(self.tasks);
        memory.timer_queue.replace(self.timers);

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
