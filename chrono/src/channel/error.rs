use std::error::Error;
use std::fmt;

// ===== Send Error =====
#[derive(Debug)]
pub struct SendError<T>(pub T);

impl<T: fmt::Debug> Error for SendError<T> {}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sending on a closed channel")
    }
}

// ===== Try Send Error =====

// Currently the same as the Send Error. This is because bounded channels
// aren't currently supported. Otherwise we would have two failure cases
#[derive(Debug)]
pub struct TrySendError<T>(pub T);

impl<T: fmt::Debug> Error for TrySendError<T> {}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sending on a closed channel")
    }
}

// ===== Try Recv Error =====

#[derive(Debug)]
pub enum TryRecvError {
    /// The channel is currently empty
    Empty,
    /// The channel's sending half is disconnected so no
    /// new messages will arrive in the channel
    Disconnected,
}

impl Error for TryRecvError {}

impl fmt::Display for TryRecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            TryRecvError::Empty => write!(f, "receiving on an empty channel"),
            TryRecvError::Disconnected => write!(f, "receiving on a closed channel"),
        }
    }
}
