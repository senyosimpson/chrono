use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use super::duration::Duration;
use super::instant::Instant;
use crate::task::waker;

pub struct Sleep {
    deadline: Instant,
}

impl Sleep {
    pub fn new(duration: Duration) -> Sleep {
        let deadline = Instant::now() + duration;
        Sleep { deadline }
    }

    pub fn done(&self) -> bool {
        Instant::now() > self.deadline
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.done() {
            Poll::Ready(())
        } else {
            let header = waker::header(cx.waker());
            unsafe { (header.vtable.schedule_timer)(waker::ptr(cx.waker()), self.deadline) }
            Poll::Pending
        }
    }
}

pub fn sleep(duration: Duration) -> Sleep {
    Sleep::new(duration)
}
