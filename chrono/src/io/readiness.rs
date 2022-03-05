use std::ops;

use super::epoll::Event;

const READABLE: usize = 1 << 0;
const WRITABLE: usize = 1 << 1;

#[derive(Clone, Copy, Default, PartialEq)]
pub struct Readiness(usize);

impl Readiness {
    pub const EMPTY: Readiness = Readiness(0);
    pub const READABLE: Readiness = Readiness(READABLE);
    pub const WRITABLE: Readiness = Readiness(WRITABLE);

    pub fn from_event(event: &Event) -> Readiness {
        let mut readiness = Readiness::EMPTY;

        if event.is_readable() {
            readiness |= Readiness::READABLE;
        }

        if event.is_writable() {
            readiness |= Readiness::WRITABLE;
        }

        readiness
    }
}

// Copied directly from Tokio
impl ops::BitOr<Readiness> for Readiness {
    type Output = Readiness;

    #[inline]
    fn bitor(self, other: Readiness) -> Readiness {
        Readiness(self.0 | other.0)
    }
}

impl ops::BitOrAssign<Readiness> for Readiness {
    #[inline]
    fn bitor_assign(&mut self, other: Readiness) {
        self.0 |= other.0;
    }
}

impl ops::BitAnd<Readiness> for Readiness {
    type Output = Readiness;

    #[inline]
    fn bitand(self, other: Readiness) -> Readiness {
        Readiness(self.0 & other.0)
    }
}

impl ops::Sub<Readiness> for Readiness {
    type Output = Readiness;

    #[inline]
    fn sub(self, other: Readiness) -> Readiness {
        Readiness(self.0 & !other.0)
    }
}
