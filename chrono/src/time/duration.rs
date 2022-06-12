use core::cmp::{PartialEq, PartialOrd};

use super::{timer::TIMER, GCD_1M};

#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub struct Duration {
    ticks: u32,
}

impl Duration {
    pub fn new(ticks: u32) -> Duration {
        Duration { ticks }
    }

    pub fn ticks(&self) -> u32 {
        self.ticks
    }

    pub fn as_secs(&self) -> u32 {
        unsafe { self.ticks / TIMER.ticks_per_second() }
    }

    pub fn from_secs(secs: u32) -> Duration {
        unsafe {
            Duration {
                ticks: secs * TIMER.ticks_per_second(),
            }
        }
    }

    pub fn as_micros(&self) -> u32 {
        unsafe { self.ticks * (1_000_000 / GCD_1M) / (TIMER.ticks_per_second() / GCD_1M) }
    }
}

impl defmt::Format for Duration {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{} seconds", self.as_secs())
    }
}
