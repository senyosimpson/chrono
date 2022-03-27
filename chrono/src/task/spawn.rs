use core::future::Future;

use crate::runtime;
use crate::runtime::Queue;
use crate::task::join::JoinHandle;
use crate::task::raw::RawTask;

pub fn spawn<F: Future>(raw: RawTask<F, Queue>) -> JoinHandle<F::Output> {
    let spawner = runtime::context::spawner();
    spawner.spawn(raw)
}
