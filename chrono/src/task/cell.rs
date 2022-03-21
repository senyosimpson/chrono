// Copied over from https://github.com/embassy-rs/embassy/blob/f683b5d4544b5b2095dd9e0530fb0569ab97967e/embassy/src/executor/raw/util.rs

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::ptr;

pub struct UninitCell<T>(MaybeUninit<UnsafeCell<T>>);

impl<T> UninitCell<T> {
    pub const fn uninit() -> Self {
        Self(MaybeUninit::uninit())
    }

    pub unsafe fn as_mut_ptr(&self) -> *mut T {
        (*self.0.as_ptr()).get()
    }

    #[allow(clippy::mut_from_ref)]
    pub unsafe fn as_mut(&self) -> &mut T {
        &mut *self.as_mut_ptr()
    }

    pub unsafe fn as_ref(&self) -> &T {
        let ptr = self.0.assume_init_ref().get();
        &*ptr
    }

    pub unsafe fn write(&self, val: T) {
        ptr::write(self.as_mut_ptr(), val)
    }

    pub unsafe fn drop_in_place(&self) {
        ptr::drop_in_place(self.as_mut_ptr())
    }
}

impl<T: Copy> UninitCell<T> {
    pub unsafe fn read(&self) -> T {
        ptr::read(self.as_mut_ptr())
    }
}