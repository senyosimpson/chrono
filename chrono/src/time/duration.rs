use core::cmp::{PartialEq, PartialOrd};

use super::timer::TIMER;

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
        unsafe {
            let gcd_1m = gcd(TIMER.ticks_per_second(), 1_000_000);
            self.ticks * (1_000_000 / gcd_1m) / (TIMER.ticks_per_second() / gcd_1m)
        }
    }

    // TODO: Add millis and micros
}

impl defmt::Format for Duration {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{} seconds", self.as_secs())
    }
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}
