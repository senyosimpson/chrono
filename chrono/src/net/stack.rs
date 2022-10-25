use core::future::{Future, poll_fn};
use core::task::{Poll, Waker};

use smoltcp::iface::Interface;

use super::devices::Enc28j60;
use crate::time::{Instant, sleep};

pub struct Stack<'a> {
    interface: Interface<'a, Enc28j60>,
    waker: Option<Waker>
}

impl<'a> Stack<'a> {
    async fn start(&mut self) {
        poll_fn(|cx| {
            // Register waker. If there is a waker, drop it
            if let Some(waker) = self.waker.take() {
                drop(waker)
            };
            self.waker = Some(cx.waker().clone());

            let timestamp = Instant::now();
            match self.interface.poll(timestamp.into()) {
                Ok(_) => {}
                Err(e) => defmt::debug!("Interface poll error: {}", e),
            };

            // If we are ready to poll the interface again, poll for more packets. Otherwise sleep
            // until the next deadline
            if let Some(deadline) = self.interface.poll_delay(timestamp.into()) {
                let delay = sleep(deadline.into());
                crate::pin!(delay);
                if delay.poll(cx).is_ready() {
                    cx.waker().wake_by_ref()
                } 
            }

            Poll::Pending
        })
        .await
    }
}
