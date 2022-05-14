//! A bounded multi-producer, single-consumer queue for sending values between
//! asynchronous tasks.

use futures_util::future::poll_fn;

use super::channel::Channel;
use crate::channel::error::{SendError, TryRecvError};

pub const fn channel<T, const N: usize>() -> Channel<T, N> {
    Channel::new()
}

/// Takes a [Channel] and splits it into Sender and Receiver halves. Each
/// half contains a reference to the channel. This avoids having to use
/// reference counting explicitly which requires allocations
pub fn split<T, const N: usize>(chan: &Channel<T, N>) -> (Sender<T, N>, Receiver<T, N>) {
    (Sender { chan }, Receiver { chan })
}

pub struct Permit<'ch, T, const N: usize> {
    chan: &'ch Channel<T, N>,
}

pub struct Sender<'ch, T, const N: usize> {
    chan: &'ch Channel<T, N>,
}

pub struct Receiver<'ch, T, const N: usize> {
    chan: &'ch Channel<T, N>,
}

// ==== impl Sender =====

impl<'ch, T, const N: usize> Sender<'ch, T, N> {
    pub async fn send(&self, message: T) -> Result<(), SendError<T>> {
        match self.reserve().await {
            Ok(permit) => permit.send(message),
            Err(_) => Err(SendError(message)),
        }
    }

    pub async fn reserve(&self) -> Result<Permit<'ch, T, N>, SendError<()>> {
        match self.chan.semaphore().acquire().await {
            Ok(_) => {
                let permit = Permit { chan: self.chan };
                Ok(permit)
            }
            Err(_) => Err(SendError(())),
        }
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

// ===== impl Permit =====

impl<'ch, T, const N: usize> Permit<'ch, T, N> {
    pub fn send(&self, message: T) -> Result<(), SendError<T>> {
        self.chan.send(message)
    }
}

impl<'ch, T, const N: usize> Drop for Permit<'ch, T, N> {
    fn drop(&mut self) {
        self.chan.semaphore().release()
    }
}
