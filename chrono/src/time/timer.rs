use std::io;
use std::os::unix::prelude::{AsRawFd, RawFd};
use std::time::Duration;

use self::timerfd::IntervalTimerSpec;

// Does not support intervals
pub(super) struct Timer {
    fd: RawFd,
}

impl Timer {
    /// Creates a new timer that fires after the length of
    /// duration has elapsed
    pub fn new(duration: Duration) -> io::Result<Timer> {
        let fd = timerfd::create()?;
        let spec = IntervalTimerSpec::new(duration);
        timerfd::set(fd, spec)?;

        Ok(Timer { fd })
    }
}

impl AsRawFd for Timer {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

/// Safe wrapper around timer_fd syscalls. It is built with the requirements
/// of this runtime in mind.
mod timerfd {
    use std::io;
    use std::os::unix::prelude::RawFd;
    use std::ptr;
    use std::time::Duration;

    #[repr(C)]
    #[derive(Default)]
    pub(super) struct IntervalTimerSpec {
        pub interval: TimeSpec,
        pub value: TimeSpec,
    }

    #[repr(C)]
    #[derive(Default)]
    pub(super) struct TimeSpec {
        pub sec: i64,
        pub nanosec: i64,
    }

    impl IntervalTimerSpec {
        pub fn new(duration: Duration) -> IntervalTimerSpec {
            let interval = TimeSpec { sec: 0, nanosec: 0 };

            let value = TimeSpec {
                sec: duration.as_secs() as i64,
                nanosec: duration.subsec_nanos() as i64,
            };

            IntervalTimerSpec { interval, value }
        }
    }

    pub(super) fn create() -> io::Result<RawFd> {
        let flags = libc::TFD_NONBLOCK | libc::TFD_CLOEXEC;
        cvt(unsafe { libc::timerfd_create(libc::CLOCK_REALTIME, flags) })
    }

    pub(super) fn set(fd: RawFd, spec: IntervalTimerSpec) -> io::Result<()> {
        let spec = &spec as *const _ as *const libc::itimerspec;
        let _ = cvt(unsafe { libc::timerfd_settime(fd, 0, spec, ptr::null_mut()) });
        Ok(())
    }

    #[allow(unused)]
    pub(super) fn get(fd: RawFd) -> io::Result<IntervalTimerSpec> {
        let mut spec = IntervalTimerSpec::default();
        let spec_ptr = &mut spec as *mut _ as *mut libc::itimerspec;
        let _ = cvt(unsafe { libc::timerfd_gettime(fd, spec_ptr) });
        Ok(spec)
    }

    #[allow(unused)]
    pub(super) fn close(fd: RawFd) -> io::Result<()> {
        cvt(unsafe { libc::close(fd) })?;
        Ok(())
    }

    // Converts C error codes into a Rust Result type
    fn cvt(result: i32) -> io::Result<i32> {
        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_timer() {
        let duration = Duration::from_secs(3);
        let timer = Timer::new(duration);
        assert!(timer.is_ok());
        let _ = timerfd::close(timer.unwrap().fd);
    }

    #[test]
    fn set_get_timer_ok() {
        let duration = Duration::from_secs(3);
        let timer = Timer::new(duration);
        assert!(timer.is_ok());

        let timer = timer.unwrap();
        let spec = timerfd::get(timer.fd).unwrap();
        let sec = spec.value.sec;
        let nanosec = spec.value.nanosec;
        let remaining_duration = Duration::new(sec as u64, nanosec as u32);

        // We know it is 2 at this stage since we started with 3 seconds and the
        // test will evaluate within a time period that this is still 2 (instead of lower)
        assert_eq!(2, sec);
        assert!(remaining_duration < duration);

        let _ = timerfd::close(timer.fd);
    }
}
