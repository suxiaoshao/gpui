use std::future::Future;

use gpui::{App, AppContext, Global, Task};

pub use tokio::task::JoinError;

pub fn init(cx: &mut App) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("failed to initialize Tokio runtime");
    let handle = runtime.handle().clone();
    cx.set_global(GlobalTokio {
        owned_runtime: Some(runtime),
        handle,
    });
}

pub fn init_from_handle(cx: &mut App, handle: tokio::runtime::Handle) {
    cx.set_global(GlobalTokio {
        owned_runtime: None,
        handle,
    });
}

struct GlobalTokio {
    owned_runtime: Option<tokio::runtime::Runtime>,
    handle: tokio::runtime::Handle,
}

impl Global for GlobalTokio {}

impl Drop for GlobalTokio {
    fn drop(&mut self) {
        if let Some(runtime) = self.owned_runtime.take() {
            runtime.shutdown_background();
        }
    }
}

pub struct Tokio;

impl Tokio {
    pub fn spawn<C, Fut, R>(cx: &C, future: Fut) -> Task<Result<R, JoinError>>
    where
        C: AppContext,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        cx.read_global(|tokio: &GlobalTokio, cx| {
            let join_handle = tokio.handle.spawn(future);
            let abort_handle = join_handle.abort_handle();
            let abort_on_drop = AbortOnDrop::new(abort_handle);
            cx.background_spawn(async move {
                let result = join_handle.await;
                drop(abort_on_drop.disarm());
                result
            })
        })
    }

    pub fn handle(cx: &App) -> tokio::runtime::Handle {
        cx.read_global(|tokio: &GlobalTokio, _| tokio.handle.clone())
    }
}

struct AbortOnDrop {
    abort_handle: Option<tokio::task::AbortHandle>,
}

impl AbortOnDrop {
    fn new(abort_handle: tokio::task::AbortHandle) -> Self {
        Self {
            abort_handle: Some(abort_handle),
        }
    }

    fn disarm(mut self) -> Self {
        self.abort_handle = None;
        self
    }
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if let Some(abort_handle) = self.abort_handle.take() {
            abort_handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SignalDrop(Option<tokio::sync::oneshot::Sender<()>>);

    impl Drop for SignalDrop {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }

    #[gpui::test]
    async fn spawn_preserves_panic_as_join_error(cx: &mut gpui::TestAppContext) {
        // Tokio wakes GPUI from another executor; allow that executor to park.
        cx.executor().allow_parking();
        cx.update(init);
        let task = cx.update(|cx| Tokio::spawn(cx, async { panic!("bridge panic contract") }));
        assert!(
            task.await
                .expect_err("panic must become JoinError")
                .is_panic()
        );
    }

    #[gpui::test]
    async fn dropping_gpui_task_aborts_a_started_tokio_future(cx: &mut gpui::TestAppContext) {
        cx.executor().allow_parking();
        cx.update(init);
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (dropped_tx, dropped_rx) = tokio::sync::oneshot::channel();
        let task = cx.update(|cx| {
            Tokio::spawn(cx, async move {
                let _signal = SignalDrop(Some(dropped_tx));
                let _ = started_tx.send(());
                std::future::pending::<()>().await;
            })
        });
        started_rx
            .await
            .expect("future must start before testing cancellation");
        drop(task);
        dropped_rx
            .await
            .expect("Task drop must abort its Tokio future");
    }

    #[gpui::test]
    async fn external_runtime_survives_bridge_teardown(cx: &mut gpui::TestAppContext) {
        cx.executor().allow_parking();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        cx.update(|cx| init_from_handle(cx, runtime.handle().clone()));
        let task = cx.update(|cx| Tokio::spawn(cx, async { 42 }));
        assert_eq!(task.await.unwrap(), 42);
        cx.update(|cx| drop(cx.remove_global::<GlobalTokio>()));
        assert_eq!(runtime.spawn(async { 43 }).await.unwrap(), 43);
    }
}
