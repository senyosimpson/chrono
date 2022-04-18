use core::future::Future;

use heapless::Arc;

use crate::runtime;
use crate::runtime::{RunQueue, SpawnError};
use crate::task::join::JoinHandle;
use crate::task::raw::RawTask;

pub fn spawn<F: Future<Output = T>, T>(raw: RawTask<F, T, Arc<RunQueue>>) -> Result<JoinHandle<T>, SpawnError> {
    let spawner = runtime::context::spawner();
    spawner.spawn(raw)
}
