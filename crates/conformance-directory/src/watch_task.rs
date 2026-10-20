use std::fmt::Display;
use std::future::Future;

use tokio::task::JoinHandle;

pub(crate) struct WatchTask<E> {
    handle: Option<JoinHandle<Result<(), E>>>,
}

impl<E: Display + Send + 'static> WatchTask<E> {
    pub(crate) fn spawn(future: impl Future<Output = Result<(), E>> + Send + 'static) -> Self {
        Self {
            handle: Some(tokio::spawn(future)),
        }
    }

    pub(crate) async fn stop(mut self) -> Option<String> {
        let handle = self.handle.take()?;
        handle.abort();
        match handle.await {
            Ok(Ok(())) => None,
            Ok(Err(e)) => Some(e.to_string()),
            Err(joined) if joined.is_cancelled() => None,
            Err(joined) => Some(joined.to_string()),
        }
    }
}

impl<E> Drop for WatchTask<E> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::oneshot;

    use super::*;

    async fn pending_with(sentinel: oneshot::Sender<()>) -> Result<(), String> {
        let _held = sentinel;
        std::future::pending().await
    }

    #[tokio::test]
    async fn dropping_the_guard_aborts_a_pending_task() {
        let (tx, rx) = oneshot::channel();
        let task = WatchTask::spawn(pending_with(tx));
        tokio::task::yield_now().await;
        drop(task);
        let closed = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .expect("the aborted task must release what it holds");
        assert!(closed.is_err(), "the sentinel is dropped, never sent");
    }

    #[tokio::test]
    async fn stopping_a_pending_task_is_not_an_error() {
        let (tx, _rx) = oneshot::channel();
        let task = WatchTask::spawn(pending_with(tx));
        assert_eq!(task.stop().await, None);
    }

    #[tokio::test]
    async fn stopping_a_task_that_already_failed_reports_the_failure() {
        let task: WatchTask<String> = WatchTask::spawn(async { Err("watch broke".to_string()) });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(task.stop().await.as_deref(), Some("watch broke"));
    }

    #[tokio::test]
    async fn stopping_a_task_that_panicked_reports_the_panic() {
        let task: WatchTask<String> = WatchTask::spawn(async { panic!("watch exploded") });
        tokio::time::sleep(Duration::from_millis(50)).await;
        let reported = task.stop().await.expect("a panic is reported");
        assert!(reported.contains("panic"), "{reported}");
    }
}
