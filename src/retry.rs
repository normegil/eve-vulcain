use std::{fmt::Debug, time::Duration};

use futures_util::Future;
use tokio::time::sleep;

pub trait RetryableError: Debug {
    fn retryable(&self) -> bool;
}

pub async fn retry<Ret, V, Err: RetryableError>(
    nb_retries: u32,
    time_wait: Duration,
    load: impl Fn() -> Ret,
) -> Result<V, Err>
where
    Ret: Future<Output = Result<V, Err>>,
{
    let mut error = None;
    for _ in 0..nb_retries {
        let res = load().await;
        match res {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                if e.retryable() {
                    sleep(time_wait).await;
                    error = Some(e);
                    continue;
                }
                return Err(e);
            }
        }
    }
    Err(error.unwrap())
}

#[cfg(test)]
pub mod mocks {
    use super::RetryableError;

    #[derive(Debug, PartialEq, Eq)]
    pub struct MockRetryableError {
        pub retryable: bool,
    }

    impl RetryableError for MockRetryableError {
        fn retryable(&self) -> bool {
            self.retryable
        }
    }
}

#[cfg(test)]
mod tests {
    use tests::mocks::MockRetryableError;

    use super::*;

    #[tokio::test]
    async fn test_retry_successful() {
        let result: Result<String, MockRetryableError> =
            retry(3, std::time::Duration::from_micros(1), async || {
                Ok("Success".to_string())
            })
            .await;
        assert_eq!(result, Ok("Success".to_string()));
    }

    #[tokio::test]
    async fn test_retry_retryable_error() {
        let retryable_load = async || {
            static mut CALL_COUNT: u32 = 0;
            unsafe {
                CALL_COUNT += 1;
                if CALL_COUNT == 1 {
                    Err(MockRetryableError { retryable: true })
                } else {
                    Ok("Success".to_string())
                }
            }
        };
        let result = retry(3, std::time::Duration::from_micros(1), retryable_load).await;
        assert_eq!(result, Ok("Success".to_string()));
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        let result: Result<(), MockRetryableError> =
            retry(3, std::time::Duration::from_micros(1), async || {
                Err(MockRetryableError { retryable: false })
            })
            .await;
        assert!(result.is_err());
    }
}
