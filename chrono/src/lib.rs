#![feature(generic_associated_types, type_alias_impl_trait)]
#![no_std]

pub mod channel;

pub mod runtime;
pub use runtime::Runtime;

pub mod task;
pub use task::spawn;
pub use task::Task;

// Re-exports
pub use chrono_macros::alloc;

pub use futures::join;
pub use futures::pin_mut as pin;
