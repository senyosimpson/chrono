use core::cell::RefCell;
use core::future::Future;
use core::marker::PhantomData;
use core::ptr::NonNull;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use stm32f3_discovery::wait_for_interrupt;

use super::context;
use super::queue::Queue;
use crate::task::join::JoinHandle;
use crate::task::RawTask;
use crate::task::Task;

pub struct Runtime {
    // Holds the task queue
    inner: RefCell<Inner>,
    // Handle to runtime
    handle: Handle,
}

struct Inner {
    /// Queue that holds tasks
    queue: *mut Queue,
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
}

// pub struct Scheduler(*mut Queue);

// ===== impl Runtime =====

impl Runtime {
    #[allow(non_upper_case_globals)]
    pub fn new() -> Runtime {
        static mut queue: Queue = Queue::new(); // "alloc" queue
        let queue_ptr = unsafe { &queue as *const _ as *mut Queue };
        let inner = RefCell::new(Inner { queue: queue_ptr });

        let spawner = Spawner { queue: queue_ptr };
        let handle = Handle { spawner };

        defmt::debug!("Queue ptr (handle): {}", handle.spawner.queue);
        defmt::debug!("Head ptr (handle): {}", unsafe {
            &(*handle.spawner.queue).head
        });
        defmt::debug!("Tail ptr (handle): {}", unsafe {
            &(*handle.spawner.queue).head
        });

        let borrow = inner.borrow();
        defmt::debug!("Queue ptr (inner): {}", borrow.queue);
        defmt::debug!("Head ptr (inner): {}", unsafe { &(*borrow.queue).head });
        defmt::debug!("Tail ptr (inner): {}", unsafe { &(*borrow.queue).tail });

        drop(borrow);

        Runtime { inner, handle }
    }

    pub fn q(&self) {
        defmt::debug!("");
        defmt::debug!("Queue ptr (handle): {}", self.handle.spawner.queue);
        defmt::debug!("Head ptr (handle): {}", unsafe {
            &(*self.handle.spawner.queue).head
        });
        defmt::debug!("Tail ptr (handle): {}", unsafe {
            &(*self.handle.spawner.queue).head
        });

        let inner = self.inner.borrow();
        defmt::debug!("Queue ptr (inner): {}", inner.queue);
        defmt::debug!("Head ptr (inner): {}", unsafe { &(*inner.queue).head });
        defmt::debug!("Tail ptr (inner): {}", unsafe { &(*inner.queue).tail });
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
                // TODO: Wrap this functionality with an interrupt processor
                // that then schedules new events
                wait_for_interrupt()
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

        // pointer to Memory inside of RawTask
        let ptr = unsafe { NonNull::new_unchecked(raw.ptr) };

        let join_handle = JoinHandle {
            raw: ptr,
            _marker: PhantomData,
        };

        // Get a pointer to our task to store in the queue
        let task = memory.task();
        let task_ptr = task as *const _ as *mut Task;
        defmt::debug!("Task {}", task_ptr);

        defmt::debug!("");
        defmt::debug!("Queue ptr (handle): {}", self.queue);
        defmt::debug!("Head ptr (handle): {}", unsafe { &(*self.queue).head });
        defmt::debug!("Tail ptr (handle): {}", unsafe { &(*self.queue).head });

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
