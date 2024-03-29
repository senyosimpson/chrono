use core::ops::{Add, Sub};

use smoltcp::time::Instant as SmoltcpInstant;
use stm32f3xx_hal::pac::DWT;

use super::duration::Duration;
use super::TICKS_PER_SECOND;

#[derive(PartialEq, Eq, PartialOrd, Clone, Copy, defmt::Format)]
pub struct Instant {
    now: u32,
}

impl Instant {
    pub const fn max() -> Instant {
        Instant { now: u32::MAX }
    }
    pub fn now() -> Instant {
        Instant {
            now: DWT::cycle_count(),
        }
    }

    pub fn from_millis(millis: u32) -> Self {
        Self {
            now: millis * (TICKS_PER_SECOND / 1000)
        }
    }

    pub fn as_millis(&self) -> u32 {
        self.now / (TICKS_PER_SECOND / 1000)
    }
}

impl Sub<Instant> for Instant {
    type Output = Duration;

    fn sub(self, rhs: Instant) -> Self::Output {
        let dur = self.now - rhs.now;
        Duration::new(dur)
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    fn add(self, rhs: Duration) -> Self::Output {
        let then = self.now + rhs.ticks();
        Instant { now: then }
    }
}


impl From<Instant> for SmoltcpInstant {
    fn from(instant: Instant) -> Self {
        SmoltcpInstant::from_millis(instant.as_millis() as i64) 
    }
}

impl From<SmoltcpInstant> for Instant {
    fn from(instant: SmoltcpInstant) -> Self {
        let millis = instant.total_millis().try_into().unwrap();
        Instant::from_millis(millis) 
    }
}