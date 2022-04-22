use core::cell::RefCell;
use core::future::Future;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use heapless::{arc_pool, Arc};

use super::context;
use super::queue::Queue;
use crate::task::join::JoinHandle;
use crate::task::Task;
use crate::task::{RawTask, Schedule};

const MAX_NUM_TASKS: usize = 1024;

pub struct Runtime {
    // Holds the task queue
    inner: RefCell<Inner>,
    // Handle to runtime
    handle: Handle,
}

struct Inner {
    /// Queue that holds tasks
    queue: Arc<RunQueue>,
}

/// Handle to the runtime
#[derive(Clone)]
pub struct Handle {
    /// Spawner responsible for spawning tasks onto the executor
    pub(crate) spawner: Spawner,
    // / Handle to the IO reactor
    // pub(crate) io: IoHandle,
}

#[derive(Clone)]
pub struct Spawner {
    queue: Arc<RunQueue>,
}

// Declare a memory pool to hold the reference-counted queues. We're
// using Arc even though it's a single-threaded executor, because there's
// no good static Rc implementation and I'm not interested in writing it
// myself lmao.
arc_pool!(RunQueue: RefCell<Queue>);

// ===== impl Runtime =====

impl Runtime {
    pub fn new() -> Runtime {
        // How we calculate the size:
        //   1. The size of the queue itself
        //   2. The size of pointers (for the reference counts). We need two: one that
        //      is stored in the runtime [Handle] and one in [Spawner]. And we need one
        //      for each task as each task stores a pointer to the queue
        const SIZE: usize = {
            let arc_size = core::mem::size_of::<Arc<RunQueue>>();
            let queue_size = core::mem::size_of::<Queue>();
            queue_size + ((MAX_NUM_TASKS + 2) * arc_size)
        };
        static mut MEMORY: [u8; SIZE] = [0; SIZE];

        // No unsafe in docs, maybe don't need it?
        unsafe { RunQueue::grow(&mut MEMORY) };

        let queue: Arc<RunQueue> = RunQueue::alloc(RefCell::new(Queue::new()))
            .ok()
            .expect("oom");
        let spawner = Spawner {
            queue: queue.clone(),
        };

        let inner = RefCell::new(Inner { queue });
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
        raw: RawTask<F, T, Arc<RunQueue>>,
    ) -> Result<JoinHandle<T>, SpawnError> {
        self.handle.spawn(raw)
    }

    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        // Enter runtime context
        let _enter = context::enter(self.handle.clone());
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

            // TODO: Block if we are waiting on something, waiting for the waker
            // to call and unblock
            
            let mut queue = self.queue.borrow_mut();
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
        raw: RawTask<F, T, Arc<RunQueue>>,
    ) -> Result<JoinHandle<T>, SpawnError> {
        self.spawner.spawn(raw)
    }
}

// ===== impl Spawner =====

pub enum SpawnError {
    QueueFull
}

impl Spawner {
    pub fn spawn<F: Future<Output = T>, T>(
        &self,
        raw: RawTask<F, T, Arc<RunQueue>>,
    ) -> Result<JoinHandle<T>, SpawnError> {
        // We need to write the scheduler into the RawTask
        let memory = raw.memory();
        let task = memory.task();
        let task_ptr = task as *const _ as *mut Task;

        unsafe { memory.scheduler.write(self.queue.clone()) }

        // pointer to Memory inside of RawTask
        let ptr = unsafe { NonNull::new_unchecked(raw.ptr) };

        let join_handle = JoinHandle {
            raw: ptr,
            _marker: PhantomData,
        };
        defmt::debug!("Task {}: Spawned", task.id);

        let spawned = self.queue.schedule(task_ptr);
        if spawned.is_err() {
            return Err(SpawnError::QueueFull)
        }

        Ok(join_handle)
    }
}

// ===== impl Queue =====

impl Schedule for Arc<RunQueue> {
    fn schedule(&self, task: *mut Task) -> Result<(), ()> {
        self.borrow_mut().insert(task);
        Ok(())
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
