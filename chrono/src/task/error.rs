use core::fmt;
use std::any::Any;

pub enum JoinError {
    Panic(Box<dyn Any + 'static>),
}

impl std::error::Error for JoinError {}

impl fmt::Display for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JoinError::Panic(_) => write!(f, "panic"),
        }
    }
}

impl fmt::Debug for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JoinError::Panic(_) => write!(f, "JoinError::Panic(..)"),
        }
    }
}
