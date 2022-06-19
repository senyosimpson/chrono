use core::cmp::{PartialEq, PartialOrd};

use super::TICKS_PER_SECOND;

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
        self.ticks / TICKS_PER_SECOND
    }

    pub fn from_secs(secs: u32) -> Duration {
        Duration {
            ticks: secs * TICKS_PER_SECOND,
        }
    }

    pub fn as_micros(&self) -> u32 {
        self.ticks
    }
}

impl defmt::Format for Duration {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{} seconds", self.as_secs())
    }
}
