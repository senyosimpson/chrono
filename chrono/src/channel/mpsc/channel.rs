use core::cell::RefCell;
use core::task::{Waker, Context, Poll};

use heapless::Deque;

use crate::channel::error::{SendError, TryRecvError};
use crate::channel::semaphore::Semaphore;


pub struct Channel<T, const N: usize> {
    /// Inner state of the channel
    inner: RefCell<Inner<T, N>>,
    /// Coordinate access to channel
    semaphore: Semaphore,
}

struct Inner<T, const N: usize> {
    /// Queue holding messages
    queue: Deque<T, N>,
    /// Number of outstanding sender handles. When it drops to
    /// zero, we close the sending half of the channel
    tx_count: usize,
    /// State of the channel
    state: State,
    /// Waker notified when items are pushed into the channel
    rx_waker: Option<Waker>,
}

enum State {
    Open,
    Closed,
}

// ===== impl Channel =====

impl<T, const N: usize> Channel<T, N> {
    pub const fn new() -> Channel<T, N> {
        Channel {
            semaphore: Semaphore::new(N),
            inner: RefCell::new(Inner {
                queue: Deque::new(), 
                tx_count: 1,
                state: State::Open,
                rx_waker: None,
            }),
        }
    }

    #[allow(unused)]
    fn wake_rx(&self) {
        let mut inner = self.inner.borrow_mut();
        if let Some(waker) = inner.rx_waker.take() {
            waker.wake();
        }
    }

    pub fn close(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.state = State::Closed;
    }

    pub fn incr_tx_count(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.tx_count += 1;
    }

    pub fn decr_tx_count(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.tx_count -= 1;
    }

    pub fn tx_count(&self) -> usize {
        self.inner.borrow().tx_count
    }

    pub fn semaphore(&self) -> &Semaphore {
        &self.semaphore
    }

    pub fn send(&self, message: T) -> Result<(), SendError<T>> {
        let mut inner = self.inner.borrow_mut();
        match inner.state {
            State::Open => {
                inner.queue.push_back(message);
                if let Some(rx_waker) = &inner.rx_waker {
                    rx_waker.wake_by_ref();
                }
                Ok(())
            }
            State::Closed => Err(SendError(message)),
        }
    }

    pub fn recv(&self, cx: &mut Context) -> Poll<Option<T>> {
        let mut inner = self.inner.borrow_mut();
        match inner.queue.pop_front() {
            // If there is a message, regardless if the channel is closed,
            // we read the message. This allows us to read any outstanding
            // messages in the event the channel is closed
            Some(message) => Poll::Ready(Some(message)),
            // If the channel is still open, then we know it's just
            // empty temporarily and could be populated in future. We
            // register the rx waker to be woken when a new task is pushed
            // into the channel.
            // If the channel is closed, then we know that no new messages
            // are coming through and we return None
            None => {
                match inner.state {
                    State::Open => {
                        // Register waker for wakeup. If there is one there, we drop it
                        // replace it with the new waker. This makes sense as we can
                        // only have one receiver waiting on the queue at a time
                        if let Some(rx_waker) = inner.rx_waker.take() {
                            drop(rx_waker)
                        }
                        inner.rx_waker = Some(cx.waker().clone());

                        Poll::Pending
                    }
                    State::Closed => Poll::Ready(None),
                }
            }
        }
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let mut inner = self.inner.borrow_mut();
        match inner.queue.pop_front() {
            Some(message) => Ok(message),
            None => match inner.state {
                State::Open => Err(TryRecvError::Empty),
                State::Closed => Err(TryRecvError::Disconnected),
            },
        }
    }
}

// SAFETY: This executor is single-threaded, thus making it safe to
// implement Sync
unsafe impl<T, const N:usize> Sync for Channel<T, N> {}