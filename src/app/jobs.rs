use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;

pub(super) fn spawn_job<T, Work, Complete>(
    tx: UnboundedSender<Action>,
    work: Work,
    complete: Complete,
) where
    T: Send + 'static,
    Work: FnOnce() -> color_eyre::Result<T> + Send + 'static,
    Complete: FnOnce(Result<T, String>) -> Action + Send + 'static,
{
    tokio::spawn(async move {
        let joined =
            tokio::task::spawn_blocking(move || work().map_err(|error| error.to_string())).await;

        let result = match joined {
            Ok(result) => result,
            Err(error) => Err(format!("Background job failed: {error}")),
        };

        let _ = tx.send(complete(result));
    });
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use color_eyre::eyre::eyre;
    use tokio::{
        sync::{mpsc, oneshot},
        time::timeout,
    };

    use super::spawn_job;
    use crate::action::Action;

    async fn run<T: Send + 'static>(
        work: impl FnOnce() -> color_eyre::Result<T> + Send + 'static,
    ) -> Result<T, String> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (result_tx, result_rx) = oneshot::channel();
        spawn_job(tx, work, move |result| {
            let _ = result_tx.send(result);
            Action::Tick
        });
        // The completion action must still reach the app's channel
        let action = timeout(Duration::from_secs(5), rx.recv()).await.unwrap();
        assert_eq!(action, Some(Action::Tick));
        result_rx.await.unwrap()
    }

    #[tokio::test]
    async fn test_spawn_job_sends_ok_result() {
        assert_eq!(run(|| Ok(42)).await, Ok(42));
    }

    #[tokio::test]
    async fn test_spawn_job_sends_error_as_string() {
        assert_eq!(
            run::<()>(|| Err(eyre!("boom"))).await,
            Err("boom".to_string())
        );
    }

    #[tokio::test]
    async fn test_spawn_job_reports_panic() {
        let result = run::<()>(|| panic!("boom")).await;
        assert!(result.unwrap_err().starts_with("Background job failed"));
    }
}
