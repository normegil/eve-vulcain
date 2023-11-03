use std::{collections::HashMap, sync::Arc};

use std::hash::Hash;
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};

use crate::{api::sde::SDEErrors, cache::FSCacheWriteError, retry::RetryableError};

use super::loader::{APIError, KeyLoader, Loader};

pub trait CacheableRetryableError:
    RetryableError + std::error::Error + Send + Sync + 'static
{
}

pub trait CacheKey = Eq + PartialEq + Hash + Clone;

pub trait IdentifiedEntryCache<K, V> {
    async fn get_or_insert<Err: CacheableRetryableError>(
        &self,
        k: &K,
        loader: &impl KeyLoader<K, V, Err>,
    ) -> Result<V, CacheError>;
}

pub trait GlobalEntryCache<V> {
    async fn get_or_insert<Err: CacheableRetryableError>(
        &self,
        loader: &impl Loader<V, Err>,
    ) -> Result<V, CacheError>;
}

#[derive(Debug, Error)]
pub enum CacheError {
    #[error(transparent)]
    Api {
        #[from]
        source: APIError,
    },
    #[error(transparent)]
    Sde {
        #[from]
        source: SDEErrors,
    },
    #[error(transparent)]
    FSCacheWrite {
        #[from]
        source: FSCacheWriteError,
    },
    #[error(transparent)]
    DataLoading {
        #[from]
        source: Arc<dyn CacheableRetryableError>,
    },
}

#[derive(Default)]
pub struct Cache<K: Eq + PartialEq + Hash, V: Clone> {
    pub memory_cache: RwLock<HashMap<K, V>>,
    nb_write: Mutex<i32>,
}

impl<K: CacheKey, V: Clone> Cache<K, V> {
    pub fn new() -> Self {
        Self {
            memory_cache: Default::default(),
            nb_write: Mutex::new(0),
        }
    }

    pub fn from(preloaded: HashMap<K, V>) -> Self {
        Self {
            memory_cache: RwLock::new(preloaded),
            nb_write: Mutex::new(0),
        }
    }
}

impl<K: CacheKey, V: Clone> IdentifiedEntryCache<K, V> for Cache<K, V> {
    async fn get_or_insert<Err: CacheableRetryableError>(
        &self,
        k: &K,
        loader: &impl KeyLoader<K, V, Err>,
    ) -> Result<V, CacheError> {
        if !self.memory_cache.read().await.contains_key(k) {
            let mut cache = self.memory_cache.write().await;
            if !cache.contains_key(k) {
                let v = loader
                    .load_with_retry(k, 5)
                    .await
                    .map_err(Arc::new)
                    .map_err(|source| CacheError::DataLoading { source })?;
                cache.insert(k.clone(), v);

                let mut nb_write = self.nb_write.lock().await;
                *nb_write += 1;
                if *nb_write >= 100 {
                    let ca = &*cache;
                    loader.persist(ca).await?;
                    *nb_write = 0;
                }
            }
        }
        Ok(self.memory_cache.read().await[k].clone())
    }
}

#[derive(Default)]
pub struct SingleCache<V: Clone> {
    pub memory_cache: RwLock<Option<V>>,
}

impl<V: Clone> SingleCache<V> {
    pub fn new() -> Self {
        Self {
            memory_cache: RwLock::new(None),
        }
    }

    pub fn from(preloaded: V) -> Self {
        Self {
            memory_cache: RwLock::new(Some(preloaded)),
        }
    }
}

impl<V: Clone> GlobalEntryCache<V> for SingleCache<V> {
    async fn get_or_insert<Err: CacheableRetryableError>(
        &self,
        loader: &impl Loader<V, Err>,
    ) -> Result<V, CacheError> {
        if self.memory_cache.read().await.is_none() {
            let mut cache = self.memory_cache.write().await;
            if cache.is_none() {
                let v = loader
                    .load()
                    .await
                    .map_err(Arc::new)
                    .map_err(|source| CacheError::DataLoading { source })?;
                *cache = Some(v);
            }
        }
        Ok(self
            .memory_cache
            .read()
            .await
            .clone()
            .expect("Cache should be already filled here"))
    }
}

#[cfg(test)]
pub mod mocks {
    use super::CacheableRetryableError;
    use crate::retry::RetryableError;
    use std::fmt::Display;

    #[derive(Debug, Clone, PartialEq)]
    pub struct MockCacheableRetryableError {
        pub msg: String,
        pub retry: bool,
    }

    impl CacheableRetryableError for MockCacheableRetryableError {}

    impl std::error::Error for MockCacheableRetryableError {}

    impl RetryableError for MockCacheableRetryableError {
        fn retryable(&self) -> bool {
            self.retry
        }
    }

    impl Display for MockCacheableRetryableError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use tests::mocks::MockCacheableRetryableError;

    use crate::api::evecache::loader::mocks::{MockGlobalLoader, MockKeyLoader};

    use super::*;

    #[tokio::test]
    async fn test_get_or_insert_from_loader() {
        let cache: SingleCache<String> = SingleCache::new();
        let loader: MockGlobalLoader<MockCacheableRetryableError> = MockGlobalLoader {
            result: Ok("test_value".to_string()),
        };

        let result = cache.get_or_insert(&loader).await;

        assert_eq!(result.unwrap(), "test_value".to_string());
    }

    #[tokio::test]
    async fn test_get_or_insert_from_cache() {
        let initial_value = "initial_value".to_string();
        let cache: SingleCache<String> = SingleCache::from(initial_value.clone());
        let loader = MockGlobalLoader {
            result: Err(MockCacheableRetryableError {
                msg: "should_not_be_called".to_string(),
                retry: false,
            }),
        };

        let result = cache.get_or_insert(&loader).await;

        assert_eq!(result.unwrap(), initial_value);
    }

    #[tokio::test]
    async fn test_get_or_insert_loader_error() {
        let cache: SingleCache<String> = SingleCache::new();
        let loader = MockGlobalLoader {
            result: Err(MockCacheableRetryableError {
                msg: "expected error".to_string(),
                retry: false,
            }),
        };

        let result = cache.get_or_insert(&loader).await;

        match result.unwrap_err() {
            CacheError::DataLoading { source: _ } => {}
            e => panic!("{:?}", e),
        }
    }

    #[tokio::test]
    async fn test_get_or_insert_cache_hit() {
        let mut cache_content = HashMap::new();
        cache_content.insert(1, "value_1".to_string());
        let cache: Cache<u32, String> = Cache::from(cache_content);
        let loader = MockKeyLoader {
            res: Some(Err(MockCacheableRetryableError {
                msg: "should not be reached".to_string(),
                retry: false,
            })),
        };

        let result = cache.get_or_insert(&1, &loader).await;

        assert_eq!(result.unwrap(), "value_1".to_string());
    }

    #[tokio::test]
    async fn test_get_or_insert_cache_miss() {
        let cache: Cache<u32, String> = Cache::new();
        let loader: MockKeyLoader<MockCacheableRetryableError> = MockKeyLoader {
            res: Some(Ok("value_1".to_string())),
        };

        let result = cache.get_or_insert(&1, &loader).await;

        assert_eq!(result.unwrap(), "value_1".to_string());
    }

    #[tokio::test]
    async fn test_keyloader_persistence() {
        let cache: Cache<u32, String> = Cache::new();
        let loader: MockKeyLoader<MockCacheableRetryableError> = MockKeyLoader { res: None };

        for i in 1..=100 {
            let result = cache.get_or_insert(&i, &loader).await;

            assert_eq!(result.unwrap(), format!("value_{}", i));
        }

        let nb_write = cache.nb_write.lock().await;
        assert_eq!(*nb_write, 0);
    }
}
