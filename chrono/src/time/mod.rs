pub(crate) mod driver;

mod duration;
pub use duration::Duration;

pub(crate) mod instant;
pub use instant::Instant;

mod sleep;
pub use sleep::sleep;

const TICKS_PER_SECOND: u32 = 1_000_000;
