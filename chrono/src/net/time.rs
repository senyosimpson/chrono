use smoltcp::time::Instant as SmoltcpInstant;
use crate::time::Instant;

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