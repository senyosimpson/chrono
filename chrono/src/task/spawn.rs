use core::future::Future;

use crate::runtime;
use crate::task::join::JoinHandle;

pub fn spawn<F: Future>(future: F) -> JoinHandle<F::Output> {
    let spawner = runtime::context::spawner();
    spawner.spawn(future)
}
