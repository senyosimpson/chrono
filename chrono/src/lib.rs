#![feature(generic_associated_types, type_alias_impl_trait, waker_getters)]
#![no_std]

pub mod channel;
pub use channel::mpsc;

pub mod runtime;
pub use runtime::Runtime;

pub mod task;
pub use task::spawn;
pub use task::Task;

pub mod time;

// Re-exports
pub use chrono_macros::alloc;

pub use futures_util::join;
pub use futures_util::pin_mut as pin;
