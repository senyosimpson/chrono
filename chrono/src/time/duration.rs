use core::cmp::{PartialEq, PartialOrd};

use smoltcp::time::Duration as SmoltcpDuration;

use super::TICKS_PER_SECOND;

#[derive(PartialEq, Eq, PartialOrd, Clone, Copy)]
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

    pub fn from_millis(millis: u32) -> Duration {
        Duration {
            ticks: millis * (TICKS_PER_SECOND / 1000)
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

impl From<SmoltcpDuration> for Duration {
    fn from(duration: SmoltcpDuration) -> Self {
        let millis = duration.total_millis().try_into().unwrap();
        Duration::from_millis(millis)
    }
}