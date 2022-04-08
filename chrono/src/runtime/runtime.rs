use core::cell::RefCell;
use core::future::Future;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use heapless::{arc_pool, Arc, Deque};

use super::context;
use crate::io::reactor::{Handle as IoHandle, Reactor};
use crate::task::join::JoinHandle;
use crate::task::Task;
use crate::task::{RawTask, Schedule};

const MAX_NUM_TASKS: usize = 1024;

pub struct Runtime {
    // Holds the reactor and task queue
    inner: RefCell<Inner>,
    // Handle to runtime
    handle: Handle,
}

struct Inner {
    /// IO reactor
    reactor: Reactor,
    /// Queue that holds tasks
    queue: Arc<RunQueue>,
}

/// Handle to the runtime
#[derive(Clone)]
pub struct Handle {
    /// Spawner responsible for spawning tasks onto the executor
    pub(crate) spawner: Spawner,
    /// Handle to the IO reactor
    pub(crate) io: IoHandle,
}

#[derive(Clone)]
pub struct Spawner {
    queue: Arc<RunQueue>,
}

pub type Queue = RefCell<Deque<Task, MAX_NUM_TASKS>>;
// Declare a memory pool to hold the reference-counted queues. We're
// using Arc even though it's a single-threaded executor, because there's
// no good static Rc implementation and I'm not interested in writing it
// myself lmao.
arc_pool!(RunQueue: Queue);

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

        let queue: Arc<RunQueue> = RunQueue::alloc(RefCell::new(Deque::new()))
            .ok()
            .expect("oom");
        let spawner = Spawner {
            queue: queue.clone(),
        };

        let reactor = Reactor::new().expect("Could not start reactor!");
        let io_handle = reactor.handle();

        // Runtime handle
        let handle = Handle {
            spawner,
            io: io_handle,
        };

        let inner = RefCell::new(Inner { reactor, queue });

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
    ) -> JoinHandle<T> {
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
                return v;
            }

            // Since we're here, we know the 'block_on' future isn't ready. We then
            // check if there have been tasks scheduled onto the runtime.
            // 1. If there are no tasks on the runtime, it means we're waiting on IO
            //    resources (e.g I'm performing a read and waiting on data to arrive).
            //    Essentially, this means we have events registered in our reactor and
            //    we are waiting for them to fire.
            // 2. If there are tasks spawned onto the runtime, we can start processing them
            if self.queue.borrow().is_empty() {
                defmt::debug!("Parking on epoll");
                self.reactor
                    .react(None)
                    .expect("Reactor failed to process events");
            }

            // We have tasks to process. We process all of them. After, we proceed to
            // to poll the outer future again with the hope that we aren't waiting on
            // anymore resources and are now finished our work (unless we are a web
            // server of course)
            loop {
                let task = self.queue.borrow_mut().pop_front();
                match task {
                    Some(task) => {
                        defmt::debug!("Task {}: Popped off executor queue and running", task.id());
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
    ) -> JoinHandle<T> {
        self.spawner.spawn(raw)
    }
}

// ===== impl Spawner =====

impl Spawner {
    pub fn spawn<F: Future<Output = T>, T>(
        &self,
        raw: RawTask<F, T, Arc<RunQueue>>,
    ) -> JoinHandle<T> {
        // We need to write the scheduler into the RawTask
        let memory = raw.memory();
        unsafe { memory.scheduler.write(self.queue.clone()) }

        let ptr = unsafe { NonNull::new_unchecked(raw.ptr) };

        let task = Task { raw: ptr };
        let join_handle = JoinHandle {
            raw: ptr,
            _marker: PhantomData,
        };
        defmt::debug!("Task {}: Spawned", task.id());

        // TODO: Figure out what to do here. This may fail. We can probably just
        // create a new SpawnError and return that
        let _ = self.queue.schedule(task);

        join_handle
    }
}

// ===== impl Queue =====

impl Schedule for Arc<RunQueue> {
    fn schedule(&self, task: Task) -> Result<(), Task> {
        self.borrow_mut().push_back(task)
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
