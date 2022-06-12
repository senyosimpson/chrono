use core::cell::RefCell;
use core::future::Future;
use core::marker::PhantomData;
use core::ptr::NonNull;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use super::queue::Queue;
use super::timer_queue::Queue as TimerQueue;
use super::{context, timer_queue};
use crate::task::join::JoinHandle;
use crate::task::RawTask;
use crate::task::Task;
use crate::time::instant::Instant;
use crate::time::timer;

pub struct Runtime {
    // Holds the task queue
    inner: RefCell<Inner>,
    // Handle to runtime
    handle: Handle,
}

struct Inner {
    /// Queue that holds tasks
    queue: *mut Queue,
    /// Queue that holds timers
    timer_queue: *mut TimerQueue,
}

/// Handle to the runtime
#[derive(Clone, Copy)]
pub struct Handle {
    /// Spawner responsible for spawning tasks onto the executor
    pub(crate) spawner: Spawner,
}

#[derive(Clone, Copy)]
pub struct Spawner {
    pub(crate) queue: *mut Queue,
    pub(crate) timer_queue: *mut timer_queue::Queue,
}

// ===== impl Runtime =====

impl Runtime {
    #[allow(non_upper_case_globals)]
    pub fn new() -> Runtime {
        let timer = timer::timer();
        timer.init();

        static mut queue: Queue = Queue::new(); // "alloc" queue
        static mut timer_queue: TimerQueue = TimerQueue::new();
        let queue_ptr = unsafe { &queue as *const _ as *mut Queue };
        let timer_queue_ptr = unsafe { &timer_queue as *const _ as *mut TimerQueue };

        let inner = RefCell::new(Inner {
            queue: queue_ptr,
            timer_queue: timer_queue_ptr,
        });

        let spawner = Spawner {
            queue: queue_ptr,
            timer_queue: timer_queue_ptr,
        };
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

            let queue = unsafe { &mut (*self.queue) };
            if queue.is_empty() {
                defmt::debug!("Queue empty. Waiting for event");
                cortex_m::asm::wfe()
            }

            // Process timers. Populates the queue with tasks that are ready to execute
            let timer_queue = unsafe { &mut (*self.timer_queue) };
            let now = Instant::now();
            timer_queue.process(now);

            // Start the timer
            // NOTE: This will cause issues because initially, it will only start timing down
            // once the first batch of tasks have been processed
            if let Some(deadline) = timer_queue.deadline() {
                let dur = deadline - Instant::now();
                timer::timer().start(dur);
                defmt::debug!("Started timer. Deadline in {}", dur);
            }

            loop {
                let task = queue.pop();
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
        memory.scheduler.replace(self.queue);
        memory.timer_queue.replace(self.timer_queue);

        // pointer to Memory inside of RawTask
        let ptr = unsafe { NonNull::new_unchecked(raw.ptr) };

        let join_handle = JoinHandle {
            raw: ptr,
            _marker: PhantomData,
        };

        // Get a pointer to our task to store in the queue
        let task = memory.task();
        let task_ptr = task as *const _ as *mut Task;

        unsafe {
            self.queue.as_mut().unwrap().push_back(task_ptr);
        }

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
