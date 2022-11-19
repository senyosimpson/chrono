#![feature(type_alias_impl_trait, waker_getters)]
#![no_std]

pub mod channel;
pub use channel::mpsc;

pub mod io {
    pub use embedded_io::asynch::{Read as AsyncRead, Write as AsyncWrite};
}

mod init;
pub use init::init;

pub mod net;

pub mod runtime;
pub use runtime::Runtime;

pub mod task;
pub use task::spawn;
pub use task::Task;

pub mod time;

// Re-exports
pub use chrono_macros::alloc;
pub use chrono_macros::main;

pub use futures_util::join;
pub use futures_util::pin_mut as pin;

pub mod hal {
    pub use stm32f3xx_hal::{delay, gpio, pac, prelude, rcc, spi, timer};
}
