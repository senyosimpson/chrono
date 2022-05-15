use core::cell::RefCell;
use core::future::Future;
use core::marker::PhantomData;
use core::mem;
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use crate::runtime::queue::Queue;

use super::cell::UninitCell;
use super::header::Header;
use super::state::State;
use super::task::Task;

#[repr(C)]
pub struct Memory<F: Future<Output = T>, T> {
    /// Header of the task. Contains data related to the state
    /// of a task
    pub header: UninitCell<Header>,
    /// Scheduler is responsible for scheduling tasks onto the
    /// runtime. When a task is woken, it calls the related
    /// scheduler to schedule itself
    pub scheduler: RefCell<*mut Queue>,
    /// The status of a task. This is either a future or the
    /// output of a future
    pub status: UninitCell<Status<F, T>>,
}

// The C representation means we have guarantees on
// the memory layout of the task
/// The underlying task containing the core components of a task
pub struct RawTask<F: Future<Output = T>, T> {
    pub ptr: *mut (),
    pub(crate) _f: PhantomData<F>,
}

pub enum Status<F: Future<Output = T>, T> {
    Running(F),
    Finished(T),
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
    fn schedule(&self, task: *mut Task) -> Result<(), ()>;
}

// ===== impl Memory ======

impl<F, T> Memory<F, T>
where
    F: Future<Output = T>,
{
    pub const fn alloc() -> Self {
        Memory {
            header: UninitCell::uninit(),
            scheduler: RefCell::new(core::ptr::null_mut()),
            status: UninitCell::uninit(),
        }
    }

    fn header(&self) -> &Header {
        unsafe { self.header.as_ref() }
    }

    pub fn task(&self) -> &Task {
        &self.header().task
    }

    #[allow(clippy::mut_from_ref)]
    unsafe fn mut_header(&self) -> &mut Header {
        self.header.as_mut()
    }

    #[allow(dead_code)]
    unsafe fn status(&self) -> &Status<F, T> {
        self.status.as_ref()
    }

    // unsafe fn scheduler(&self) -> *mut Queue {
    //     self.scheduler
    // }

    #[allow(clippy::mut_from_ref)]
    unsafe fn mut_status(&self) -> &mut Status<F, T> {
        self.status.as_mut()
    }
}

unsafe impl<F: Future<Output = T>, T> Sync for Memory<F, T> {}

// ===== impl RawTask =====

impl<F, T> RawTask<F, T>
where
    F: Future<Output = T>,
{
    // What implication is there for having a const within an impl? Is that the same
    // as having it outside?
    const RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
        Self::clone_waker,
        Self::wake,
        Self::wake_by_ref,
        Self::drop_waker,
    );

    pub fn new(memory: &Memory<F, T>, future: impl FnOnce() -> F) -> RawTask<F, T> {
        let ptr = memory as *const _ as *mut ();

        let task = Task::new(unsafe { NonNull::new_unchecked(ptr) });
        let task_id = task.id;

        let header = Header {
            task: task,
            state: State::new_with_id(task_id),
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

        RawTask {
            ptr,
            _f: PhantomData,
        }
    }

    pub(crate) fn memory(&self) -> &Memory<F, T> {
        unsafe { &*(self.ptr as *const Memory<F, T>) }
    }

    fn from_ptr(ptr: *const ()) -> Self {
        let ptr = ptr as *mut ();
        Self {
            ptr,
            _f: PhantomData,
        }
    }

    // TODO: No deallocations in embedded so we can remove this
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn dealloc(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let task = memory.task();
        defmt::debug!("Task {}: Deallocating", task.id);
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

        let task = memory.task();
        defmt::debug!("Task {}: Waking raw task", task.id);

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

        let task = memory.task();
        defmt::debug!("Task {}: Waking raw task by ref", task.id);

        header.state.transition_to_scheduled();
        Self::schedule(ptr);
    }

    unsafe fn schedule(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let header = memory.mut_header();

        // header.task.set(NonNull::new_unchecked(ptr as *mut ()));
        // When we create a new task, we need to increment its reference
        // count since we now have another 'thing' holding a reference
        // to the raw task
        header.state.ref_incr();

        let task_ptr = memory.task() as *const _ as *mut Task;
        // let scheduler = &mut (*memory.scheduler());
        let scheduler = memory.scheduler.borrow_mut();
        // TODO We need to store that a task failed to be scheduled in the
        // state or something of that kind
        let _ = scheduler.as_mut().unwrap().push_back(task_ptr);
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
                defmt::debug!("Task pending");
                header.state.transition_to_idle();
            }
            Poll::Ready(_) => {
                header.state.transition_to_complete();
                if header.state.has_join_waker() {
                    header.wake_join_handle();
                } else {
                    // Drop the future or output by replacing it with Consumed
                    status.drop_future_or_output();
                }
            }
        }
    }

    fn poll_inner(status: &mut Status<F, T>, cx: &mut Context) -> Poll<()> {
        let res = status.poll(cx);

        let output = match res {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(output) => output,
        };

        // Store output in task
        *status = Status::Finished(output);

        Poll::Ready(())
    }

    unsafe fn get_output(ptr: *const (), dst: *mut ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let status = memory.mut_status();
        let dst = dst as *mut Poll<F::Output>;
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

impl<F: Future<Output = T>, T> Status<F, T> {
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
