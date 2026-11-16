use std::io;

use tokio::runtime::{Builder, Runtime};

const ASYNC_WORKER_THREADS: usize = 4;
const BLOCKING_POOL_THREADS: usize = 16;

pub struct CompactTokioExecutor {
    tokio_runtime: Runtime,
}

impl iced::Executor for CompactTokioExecutor {
    fn new() -> Result<Self, io::Error> {
        Builder::new_multi_thread()
            .worker_threads(ASYNC_WORKER_THREADS)
            .max_blocking_threads(BLOCKING_POOL_THREADS)
            .enable_all()
            .build()
            .map(|tokio_runtime| Self { tokio_runtime })
    }

    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        drop(self.tokio_runtime.spawn(future));
    }

    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        self.tokio_runtime.block_on(future)
    }

    fn enter<R>(&self, work: impl FnOnce() -> R) -> R {
        let _runtime_guard = self.tokio_runtime.enter();
        work()
    }
}
