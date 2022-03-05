use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::ready;

use super::timer::Timer;
use crate::io::pollable::Pollable;

// Future that is returned from a call to `sleep`
pub struct Sleep {
    inner: Pollable<Timer>,
}

impl Sleep {
    fn until(duration: Duration) -> Sleep {
        // TODO: Saner error handling
        let timer = Timer::new(duration).unwrap();
        let inner = Pollable::new(timer).unwrap();
        Sleep { inner }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // TODO: Improve error handling
        match ready!(self.inner.poll_readable(cx)) {
            Ok(()) => Poll::Ready(()),
            Err(e) => panic!("timer error: {}", e),
        }
    }
}

pub fn sleep(duration: Duration) -> Sleep {
    Sleep::until(duration)
}
