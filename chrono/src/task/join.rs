use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, Poll};

use crate::task::header::Header;

/// A handle to the task
pub struct JoinHandle<T> {
    /// Pointer to raw task
    pub(crate) raw: NonNull<()>,
    pub(crate) _marker: PhantomData<T>,
}

impl<T> Future for JoinHandle<T> {
    type Output = super::Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let raw = self.raw.as_ptr();
        let mut output = Poll::Pending;

        unsafe {
            let header = &mut *(raw as *mut Header);

            let id = header.id;
            tracing::debug!(
                "Task {}: JoinHandle is complete: {}",
                id,
                header.state.is_complete()
            );

            if !header.state.is_complete() {
                // Register waker with the task
                header.register_waker(cx.waker());
                header.state.set_join_waker();
            } else {
                tracing::debug!("Task {}: JoinHandle ready", id);
                (header.vtable.get_output)(self.raw.as_ptr(), &mut output as *mut _ as *mut ());
            }
        }

        output
    }
}

impl<T> Drop for JoinHandle<T> {
    fn drop(&mut self) {
        let raw = self.raw.as_ptr();
        let header = raw as *mut Header;

        unsafe {
            tracing::debug!("Task {}: Dropping JoinHandle", ((*header).id));
            ((*header).vtable.drop_join_handle)(self.raw.as_ptr())
        }
    }
}
