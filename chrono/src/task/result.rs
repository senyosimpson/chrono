use super::error::JoinError;

/// Task result sent back.
pub(crate) type Result<T> = std::result::Result<T, JoinError>;
