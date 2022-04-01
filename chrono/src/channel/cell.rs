use once_cell::unsync::OnceCell;

/// A wrapper around [unsync::OnceCell] that makes it `sync`. This is
/// useful because the runtime is single-threaded and we don't want/need
/// to pay the price of atomics
pub struct StaticCell<T>(OnceCell<T>);

// SAFETY: Only used in a single-threaded scenario so safe to implement
unsafe impl<T> Sync for StaticCell<T> {}

impl<T> StaticCell<T> {
    pub const fn new() -> StaticCell<T> {
        StaticCell(OnceCell::new())
    }

    /// Set the value of the cell. Returns a reference to the
    /// value
    pub fn set(&self, value: T) -> &T {
        let _ = self.0.set(value);
        self.0.get().unwrap()
    }
}
