use std::ptr::NonNull;

use super::header::{Header, TaskId};

pub(crate) struct Task {
    pub(crate) raw: NonNull<()>,
}

impl Task {
    pub fn id(&self) -> TaskId {
        let ptr = self.raw.as_ptr();
        let header = ptr as *const Header;
        unsafe { (*header).id }
    }

    pub fn run(self) {
        let ptr = self.raw.as_ptr();
        let header = ptr as *const Header;
        unsafe { ((*header).vtable.poll)(ptr) }
    }
}
