use core::fmt;

// ===== Send Error =====
#[derive(Debug)]
pub enum SendError<T> {
    Full(T),
    Closed(T),
}

impl<T> fmt::Display for SendError<T> {
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

impl fmt::Display for TryRecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            TryRecvError::Empty => write!(f, "receiving on an empty channel"),
            TryRecvError::Disconnected => write!(f, "receiving on a closed channel"),
        }
    }
}
