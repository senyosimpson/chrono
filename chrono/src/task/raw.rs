use core::future::Future;
use core::marker::PhantomData;
use core::mem;
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use std::panic;

use super::cell::UninitCell;
use super::error::JoinError;
use super::header::{Header, TaskId};
use super::state::State;
use super::task::Task;

#[repr(C)]
pub struct Memory<F: Future, S> {
    /// Header of the task. Contains data related to the state
    /// of a task
    pub header: UninitCell<Header>,
    /// Scheduler is responsible for scheduling tasks onto the
    /// runtime. When a task is woken, it calls the related
    /// scheduler to schedule itself
    pub scheduler: UninitCell<S>,
    /// The status of a task. This is either a future or the
    /// output of a future
    pub status: UninitCell<Status<F>>,
}

// The C representation means we have guarantees on
// the memory layout of the task
/// The underlying task containing the core components of a task
pub struct RawTask<F: Future, S> {
    pub ptr: *mut (),
    pub(crate) _f: PhantomData<F>,
    pub(crate) _s: PhantomData<S>,
}

pub enum Status<F: Future> {
    Running(F),
    Finished(super::Result<F::Output>),
    Consumed,
}

pub struct TaskVTable {
    pub(crate) poll: unsafe fn(*const ()),
    pub(crate) get_output: unsafe fn(*const (), *mut ()),
    pub(crate) drop_join_handle: unsafe fn(*const ()),
}

// All schedulers must implement the Schedule trait. They
// are responsible for sending tasks to the runtime queue
pub trait Schedule {
    fn schedule(&self, task: Task) -> Result<(), Task>;
}

// ===== impl Memory ======

impl<F, S> Memory<F, S>
where
    F: Future,
    S: Schedule,
{
    pub const fn alloc() -> Self {
        Memory {
            header: UninitCell::uninit(),
            scheduler: UninitCell::uninit(),
            status: UninitCell::uninit(),
        }
    }

    fn header(&self) -> &Header {
        unsafe {
            self.header.as_ref()
        }
    }

    #[allow(clippy::mut_from_ref)]
    unsafe fn mut_header(&self) -> &mut Header{
        self.header.as_mut()
    }

    #[allow(dead_code)]
    unsafe fn status(&self) -> &Status<F> {
        self.status.as_ref()
    }

    unsafe fn scheduler(&self) -> &S {
        self.scheduler.as_ref()
    }

    #[allow(clippy::mut_from_ref)]
    unsafe fn mut_status(&self) -> &mut Status<F> {
        self.status.as_mut()
    }
}

unsafe impl<F: Future, S> Sync for Memory<F, S> {}

// ===== impl RawTask =====

impl<F, S> RawTask<F, S>
where
    F: Future,
    S: Schedule,
{
    // What implication is there for having a const within an impl? Is that the same
    // as having it outside?
    const RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
        Self::clone_waker,
        Self::wake,
        Self::wake_by_ref,
        Self::drop_waker,
    );

    pub fn new(memory: &Memory<F, S>, future: impl FnOnce() -> F) -> RawTask<F, S> {
        let id = TaskId::new();

        let header = Header {
            id,
            state: State::new_with_id(id),
            waker: None,
            vtable: &TaskVTable {
                poll: Self::poll,
                get_output: Self::get_output,
                drop_join_handle: Self::drop_join_handle,
            },
        };

        // NOTE: The scheduler is written when a task is spawned
        // TODO: Should I hide safety in UninitCell?
        unsafe { memory.header.write(header) }

        let status = Status::Running(future());
        unsafe { memory.status.write(status) };

        // let ptr =  unsafe { NonNull::new_unchecked(memory as *const _ as *mut ()) };
        // let ptr = memory as *const _;

        let ptr = memory as *const _ as *mut ();
        RawTask {
            ptr,
            _f: PhantomData,
            _s: PhantomData,
        }
    }

    pub(crate) fn memory(&self) -> &Memory<F, S> {
        unsafe { &*(self.ptr as *const Memory<F, S>) }
    }

    fn from_ptr(ptr: *const ()) -> Self {
        let ptr = ptr as *mut ();
        Self {
            ptr,
            _f: PhantomData,
            _s: PhantomData,
        }
    }

    // TODO: No deallocations in embedded so we can remove this
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn dealloc(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let header = memory.header();
        tracing::debug!("Task {}: Deallocating", header.id);
    }

    // Makes a clone of the waker
    // Increments the number of references to the waker
    unsafe fn clone_waker(ptr: *const ()) -> RawWaker {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let header = memory.mut_header();

        header.state.ref_incr();
        RawWaker::new(ptr, &Self::RAW_WAKER_VTABLE)
    }

    // This is responsible for decrementing a reference count and ensuring
    // the task is destroyed if the reference count is 0
    unsafe fn drop_waker(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let header = memory.mut_header();

        header.state.ref_decr();
        if header.state.ref_count() == 0 {
            Self::dealloc(ptr)
        }
    }

    /// Wakes the task
    // One requirement here is that it must be safe
    // to call `wake` even if the task has been driven to completion
    unsafe fn wake(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let header = memory.mut_header();
        tracing::debug!("Task {}: Waking raw task", header.id);

        header.state.transition_to_scheduled();
        // We get one reference count from the caller. We schedule a task which
        // increases our reference count by one.
        Self::schedule(ptr);
        // We can now drop our reference from the caller
        Self::drop_waker(ptr);
    }

    unsafe fn wake_by_ref(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let header = memory.mut_header();
        tracing::debug!("Task {}: Waking raw task by ref", header.id);

        header.state.transition_to_scheduled();
        Self::schedule(ptr);
    }

    unsafe fn schedule(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let header = memory.mut_header();

        let task = Task {
            raw: NonNull::new_unchecked(ptr as *mut ()),
        };
        // When we create a new task, we need to increment its reference
        // count since we now have another 'thing' holding a reference
        // to the raw task
        header.state.ref_incr();

        let scheduler = memory.scheduler();
        // TODO We need to store that a task failed to be scheduled in the
        // state or something of that kind
        let _ = scheduler.schedule(task);
    }

    // Runs the future and updates its state
    unsafe fn poll(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let header = memory.mut_header();

        let waker = Waker::from_raw(RawWaker::new(ptr, &Self::RAW_WAKER_VTABLE));
        let cx = &mut Context::from_waker(&waker);

        header.state.transition_to_running();

        let status = memory.mut_status();
        match Self::poll_inner(status, cx) {
            Poll::Pending => {
                tracing::debug!("Task pending");
                header.state.transition_to_idle();
            }
            Poll::Ready(_) => {
                header.state.transition_to_complete();
                // Catch a panic if waking the JoinHandle or dropping the future
                // panics. Since the task is already completed, we're not concerned
                // about propagating the failure up to the caller
                let _ = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    if header.state.has_join_waker() {
                        header.wake_join_handle();
                    } else {
                        // Drop the future or output by replacing it with Consumed
                        status.drop_future_or_output();
                    }
                }));
            }
        }
    }

    fn poll_inner(status: &mut Status<F>, cx: &mut Context) -> Poll<()> {
        struct Guard<'a, F: Future> {
            status: &'a mut Status<F>,
        }

        impl<'a, F: Future> Drop for Guard<'a, F> {
            fn drop(&mut self) {
                // If polling the future panics, we want to drop the future/output
                // If dropping the future/output panics, we've wrapped the entire method in
                // a panic::catch_unwind so we can return a JoinError
                self.status.drop_future_or_output()
            }
        }

        let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            let guard = Guard { status };
            let res = guard.status.poll(cx);
            // Successfully polled the future. Prevent the guard's destructor from running
            mem::forget(guard);
            res
        }));

        let output = match res {
            Ok(Poll::Pending) => return Poll::Pending,
            Ok(Poll::Ready(output)) => Ok(output),
            Err(panic) => Err(JoinError::Panic(panic)),
        };

        // Store output in task. Ignore if the future panics on drop
        let _ = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            *status = Status::Finished(output);
        }));

        Poll::Ready(())
    }

    unsafe fn get_output(ptr: *const (), dst: *mut ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let status = memory.mut_status();
        let dst = dst as *mut Poll<super::Result<F::Output>>;
        // TODO: Improve error handling
        match mem::replace(status, Status::Consumed) {
            Status::Finished(output) => {
                *dst = Poll::Ready(output);
            }
            _ => panic!("Could not retrieve output!"),
        }
    }

    unsafe fn drop_join_handle(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let header = memory.mut_header();

        // unset join handle bit
        header.state.unset_join_handle();
        // drop the reference the handle was holding, possibly
        // deallocating the task
        header.state.ref_decr();
        if header.state.ref_count() == 0 {
            Self::dealloc(ptr)
        }
    }
}

// ====== impl Status =====

impl<F: Future> Status<F> {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<F::Output> {
        let future = match self {
            Status::Running(future) => future,
            _ => unreachable!("unexpected status"),
        };

        let future = unsafe { Pin::new_unchecked(future) };
        future.poll(cx)

        // if res.is_ready() { self.drop_future_or_output() }
    }

    fn drop_future_or_output(&mut self) {
        *self = Status::Consumed
    }
}
