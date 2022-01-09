use alloc::vec::Vec;
use core::future::Future;
use nostd_async::{JoinHandle, Runtime, Task};
use KRdmaKit::rust_kernel_rdma_base::*;
use linux_kernel_module::println;

async fn async_run(task_idx: usize) -> u32 {
    for section_index in 1..=10 {
        println!("async Task {} Section {}", task_idx, section_index);
        // yield the target to the end of waiting queue
        futures_micro::yield_once().await;
    }
    1
}

pub fn xx() {
    let len = 20;
    let runtime = Runtime::new();
    let mut t1 = Task::new(async move {
        async_run(0).await
    });

    let mut t2 = Task::new(async move {
        async_run(1).await
    });

    let mut t3 = Task::new(async move {
        async_run(3).await
    });

    let h1 = t1.spawn(&runtime);
    let h3 = t3.spawn(&runtime);
    let h2 = t2.spawn(&runtime);
    h1.join();
}

///
/// Runtime Wrapper.
/// ```
/// let mut task = nostd_async::Task::new(async move {
///        println!("hello world!");
///        0
///  });
/// let mut async_rt = AsyncRuntime::create();
/// let handler = async_rt.spawn_task(&mut task);
/// handler.join();
/// ```
///
pub struct AsyncRuntime {
    runtime: nostd_async::Runtime,

}

impl<'t> AsyncRuntime {
    pub fn create() -> Self {
        Self {
            runtime: nostd_async::Runtime::new()
        }
    }

    pub fn spawn_task<T>(&'t self,
                         task: &'t mut nostd_async::Task<'t,
                             impl Future<Output=T>, T>,
    ) -> JoinHandle<'t, T> {
        task.spawn(&self.runtime)
    }
}

