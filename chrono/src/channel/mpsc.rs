//! A multi-producer, single-consumer queue for sending values between
//! asynchronous tasks.

use core::cell::RefCell;
use core::task::{Context, Poll, Waker};

use futures::future::poll_fn;
use heapless::Deque;

use super::cell::StaticCell;
use super::error::{SendError, TryRecvError};

/// Takes a [Channel] and splits it into Sender and Receiver halves. Each
/// half contains a reference to the channel. This avoids having to use
/// reference counting explicitly which requires allocations
pub fn split<T, const N: usize>(chan: &Channel<T, N>) -> (Sender<T, N>, Receiver<T, N>) {
    (Sender { chan }, Receiver { chan })
}

/// Holds a [Channel]. Really the purpose of this is to create a more pleasant
/// API when initialising static [Channel]s.
pub struct ChannelCell<T, const N: usize>(StaticCell<Channel<T, N>>);

impl<T, const N: usize> ChannelCell<T, N> {
    pub const fn new() -> ChannelCell<T, N> {
        ChannelCell(StaticCell::new())
    }

    pub fn set(&self, channel: Channel<T, N>) -> &Channel<T, N> {
        self.0.set(channel)
    }
}

pub struct Sender<'ch, T, const N: usize> {
    chan: &'ch Channel<T, N>,
}

pub struct Receiver<'ch, T, const N: usize> {
    chan: &'ch Channel<T, N>,
}

pub struct Channel<T, const N: usize> {
    inner: RefCell<Inner<T, N>>,
}

struct Inner<T, const N: usize> {
    // Queue holding messages
    queue: Deque<T, N>,
    // Number of outstanding sender handles. When it drops to
    // zero, we close the sending half of the channel
    tx_count: usize,
    // State of the channel
    state: State,
    // Waker notified when items are pushed into the channel
    rx_waker: Option<Waker>,
}

enum State {
    Open,
    Closed,
}

// ==== impl Sender =====

impl<'ch, T, const N: usize> Sender<'ch, T, N> {
    pub fn send(&self, message: T) -> Result<(), SendError<T>> {
        self.chan.send(message)
    }
}

impl<'ch, T, const N: usize> Clone for Sender<'ch, T, N> {
    fn clone(&self) -> Self {
        self.chan.incr_tx_count();
        Self { chan: self.chan }
    }
}

impl<'ch, T, const N: usize> Drop for Sender<'ch, T, N> {
    fn drop(&mut self) {
        defmt::debug!("Dropping sender");
        self.chan.decr_tx_count();
        if self.chan.tx_count() == 0 {
            self.chan.close();
        }
    }
}

// ===== impl Receiver =====

impl<'ch, T, const N: usize> Receiver<'ch, T, N> {
    pub async fn recv(&self) -> Option<T> {
        poll_fn(|cx| self.chan.recv(cx)).await
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.chan.try_recv()
    }
}

impl<'ch, T, const N: usize> Drop for Receiver<'ch, T, N> {
    fn drop(&mut self) {
        defmt::debug!("Dropping receiver");
        self.chan.close();
    }
}

// ===== impl Channel =====

impl<T, const N: usize> Channel<T, N> {
    pub fn new() -> Channel<T, N> {
        Channel {
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

    fn close(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.state = State::Closed;
    }

    fn incr_tx_count(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.tx_count += 1;
    }

    fn decr_tx_count(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.tx_count -= 1;
    }

    fn tx_count(&self) -> usize {
        self.inner.borrow().tx_count
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
