mod duration;
pub use duration::Duration;

pub(crate) mod instant;
pub use instant::Instant;

mod sleep;
pub use sleep::sleep;

pub mod timer;

const TICKS_PER_SECOND: u32 = 1_000_000;
const GCD_1M: u32 = gcd(TICKS_PER_SECOND, 1_000_000);

// Copied from embassy
const fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}
