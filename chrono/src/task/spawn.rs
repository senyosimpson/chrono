use core::future::Future;

use heapless::Arc;

use crate::runtime;
use crate::runtime::RunQueue;
use crate::task::join::JoinHandle;
use crate::task::raw::RawTask;

pub fn spawn<F: Future>(raw: RawTask<F, Arc<RunQueue>>) -> JoinHandle<F::Output> {
    let spawner = runtime::context::spawner();
    spawner.spawn(raw)
}
