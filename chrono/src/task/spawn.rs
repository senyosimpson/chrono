use core::future::Future;

use crate::runtime::{context, SpawnError};
use crate::task::join::JoinHandle;
use crate::task::raw::Permit;

pub fn spawn<F: Future<Output = T>, T>(permit: Permit<F, T>) -> Result<JoinHandle<T>, SpawnError> {
    let spawner = context::spawner();
    spawner.spawn(permit)
}
