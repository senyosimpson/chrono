use core::cell::RefCell;
use core::future::Future;
use core::marker::PhantomData;
use core::mem;
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use crate::runtime::queue::Queue;
use crate::runtime::timer_queue;
use crate::time::Instant;

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
    pub timer_queue: RefCell<*mut timer_queue::Queue>,
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
    pub(crate) schedule: unsafe fn(*const ()),
    pub(crate) schedule_timer: unsafe fn(*const (), Instant),
    pub(crate) get_output: unsafe fn(*const (), *mut ()),
    pub(crate) drop_join_handle: unsafe fn(*const ()),
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
            timer_queue: RefCell::new(core::ptr::null_mut()),
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
    const RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
        Self::clone_waker,
        Self::wake,
        Self::wake,
        Self::drop_waker,
    );

    pub fn new(memory: &Memory<F, T>, future: impl FnOnce() -> F) -> RawTask<F, T> {
        let ptr = memory as *const _ as *mut ();

        let task = Task::new(unsafe { NonNull::new_unchecked(ptr) });
        let task_id = task.id;

        let header = Header {
            task,
            state: State::new_with_id(task_id),
            timer_expiry: None,
            waker: None,
            vtable: &TaskVTable {
                poll: Self::poll,
                schedule: Self::schedule,
                schedule_timer: Self::schedule_timer,
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

    unsafe fn clone_waker(ptr: *const ()) -> RawWaker {
        RawWaker::new(ptr, &Self::RAW_WAKER_VTABLE)
    }

    unsafe fn drop_waker(_: *const ()) {
        // no op
    }

    /// Wakes the task
    unsafe fn wake(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let header = memory.mut_header();

        let task = memory.task();
        defmt::debug!("Task {}: Waking raw task", task.id);

        header.state.transition_to_scheduled();
        Self::schedule(ptr);
    }

    unsafe fn schedule(ptr: *const ()) {
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();

        let task_ptr = memory.task() as *const _ as *mut Task;
        let scheduler = memory.scheduler.borrow_mut();
        scheduler.as_mut().unwrap().push_back(task_ptr);
    }

    unsafe fn schedule_timer(ptr: *const (), deadline: Instant) {
        defmt::debug!("Timer scheduled. Expiry at {}", deadline);
        let raw = Self::from_ptr(ptr);
        let memory = raw.memory();
        let header = memory.mut_header();

        header.timer_expiry = Some(deadline);

        let task_ptr = memory.task() as *const _ as *mut Task;
        let timer_queue = memory.timer_queue.borrow_mut();
        // TODO We need to store that a task failed to be scheduled in the
        // state or something of that kind
        timer_queue.as_mut().unwrap().push_back(task_ptr);
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
