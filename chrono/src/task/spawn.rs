use core::future::Future;

use heapless::Arc;

use crate::runtime;
use crate::runtime::RunQueue;
use crate::task::join::JoinHandle;
use crate::task::raw::RawTask;

pub fn spawn<F: Future<Output = T>, T>(raw: RawTask<F, T, Arc<RunQueue>>) -> JoinHandle<T> {
    let spawner = runtime::context::spawner();
    spawner.spawn(raw)
}
