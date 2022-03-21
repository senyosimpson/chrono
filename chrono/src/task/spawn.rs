use core::future::Future;

use crate::runtime::Queue;
use crate::{runtime, RawTask};
use crate::task::join::JoinHandle;

pub fn spawn<F: Future>(raw: RawTask<F, Queue>) -> JoinHandle<F::Output> {
    let spawner = runtime::context::spawner();
    spawner.spawn(raw)
}
