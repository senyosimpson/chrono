use once_cell::unsync::OnceCell;
pub struct StaticCell<T>(OnceCell<T>);

// SAFETY: Only used in a single-threaded scenario so safe to implement
unsafe impl<T> Sync for StaticCell<T> {}

impl<T> StaticCell<T> {
    pub const fn new() -> StaticCell<T> {
        StaticCell(OnceCell::new())
    }

    pub fn set(&self, value: T) -> &T {
        let _ = self.0.set(value);
        self.0.get().unwrap()
    }
}
