//! Shared utility functions.

use std::time::Duration;

use tokio::task::spawn_blocking;
use tokio::time::timeout;

use crate::{Error, Result};

pub async fn blocking<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    spawn_blocking(f)
        .await
        .map_err(|e| Error::TaskJoin(e.to_string()))?
}

pub async fn blocking_with_timeout<F, T>(duration: Duration, f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    match timeout(duration, spawn_blocking(f)).await {
        Ok(Ok(inner)) => inner,
        Ok(Err(join_err)) => Err(Error::TaskJoin(join_err.to_string())),
        Err(_) => Err(Error::Timeout(duration)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_blocking_success() {
        assert_eq!(blocking(|| Ok::<_, Error>(42)).await.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_blocking_with_timeout_success() {
        assert_eq!(
            blocking_with_timeout(Duration::from_secs(1), || Ok::<_, Error>(42))
                .await
                .unwrap(),
            42
        );
    }

    #[tokio::test]
    async fn test_blocking_with_timeout_expires() {
        let result = blocking_with_timeout(Duration::from_millis(10), || {
            std::thread::sleep(Duration::from_millis(100));
            Ok::<_, Error>(42)
        })
        .await;
        assert!(matches!(result.unwrap_err(), Error::Timeout(_)));
    }
}
