use std::path::PathBuf;

use chrono::{Duration, Local, LocalResult, NaiveDateTime, TimeZone};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::display::Display;
use crate::logging;

static CACHE_TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Error, Debug)]
pub enum FSCacheReadError {
    #[error("read file '{path}': {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },
    #[error("deserialize json content of '{path}': {source}")]
    JSONDeserialization {
        path: String,
        source: serde_json::Error,
    },
    #[error("load cache date '{date}': {source}")]
    InvalidCacheDate {
        date: String,
        source: chrono::ParseError,
    },
    #[error("fail to transform cache date '{date}' to local date")]
    CacheDateToLocalDate { date: String },
}

#[derive(Error, Debug)]
pub enum FSCacheWriteError {
    #[error("write file '{path}': {source}")]
    WriteFileError {
        path: String,
        source: std::io::Error,
    },
    #[error("serialize json content of '{path}': {source}")]
    JSONSeserializationError {
        path: String,
        source: serde_json::Error,
    },
}

#[derive(Clone)]
pub struct FSCache {
    pub cache_directory: PathBuf,
}

impl FSCache {
    pub fn new(cache_directory: PathBuf) -> Self {
        logging::trace!("Cache directory: {}", cache_directory.to_display());
        FSCache { cache_directory }
    }
}

impl FSCache {
    pub async fn load_full(&self, path: &str) -> Result<String, FSCacheReadError> {
        let mut file = self.cache_directory.clone();
        file.push(path);
        let content = tokio::fs::read_to_string(&file).await.map_err(|source| {
            FSCacheReadError::ReadFile {
                path: file.to_display(),
                source,
            }
        })?;
        Ok(content)
    }

    pub async fn load_from_cache<T: DeserializeOwned>(
        &self,
        name: CacheName,
        validity_period: Duration,
    ) -> Result<Option<T>, FSCacheReadError> {
        let mut file = self.cache_directory.clone();
        file.push(name.to_string() + "_cache.json");
        if !file.exists() {
            logging::trace!("No cache file found: '{}'", name);
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(&file).await.map_err(|source| {
            FSCacheReadError::ReadFile {
                path: file.to_display(),
                source,
            }
        })?;
        let cache: TimedCache<Option<T>> = serde_json::from_str(&content).map_err(|source| {
            FSCacheReadError::JSONDeserialization {
                path: file.to_display(),
                source,
            }
        })?;

        let registered_time =
            NaiveDateTime::parse_from_str(&cache.registered_time, CACHE_TIME_FORMAT).map_err(
                |source| FSCacheReadError::InvalidCacheDate {
                    date: cache.registered_time,
                    source,
                },
            )?;
        let registered_time = match Local.from_local_datetime(&registered_time) {
            LocalResult::None => {
                return Err(FSCacheReadError::CacheDateToLocalDate {
                    date: registered_time.to_string(),
                });
            }
            LocalResult::Single(time) => time,
            LocalResult::Ambiguous(_, max_time) => max_time,
        };

        let cache_age = Local::now() - registered_time;
        if cache_age >= validity_period {
            logging::trace!("Cache expired: '{}'", name);
            return Ok(None);
        }

        match cache.cached_data {
            None => {
                logging::trace!("No cache data: '{}'", name);
                Ok(None)
            }
            Some(data) => {
                logging::debug!("Cache loaded: '{}'", name);
                Ok(Some(data))
            }
        }
    }

    pub fn save_to_cache<T: Serialize>(
        &self,
        name: CacheName,
        data: T,
    ) -> Result<(), FSCacheWriteError> {
        let mut file = self.cache_directory.clone();
        file.push(name.to_string() + "_cache.json");

        let content = serde_json::to_string(&TimedCache {
            registered_time: format!("{}", Local::now().format(CACHE_TIME_FORMAT)),
            cached_data: data,
        })
        .map_err(|source| FSCacheWriteError::JSONSeserializationError {
            path: file.to_display(),
            source,
        })?;
        std::fs::write(&file, content).map_err(|source| FSCacheWriteError::WriteFileError {
            path: file.to_display(),
            source,
        })?;
        logging::debug!("Cache saved: '{}'", name);
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct TimedCache<T> {
    registered_time: String,
    cached_data: T,
}

#[derive(Debug)]
pub enum CacheName {
    Constellations,
    IndustrialSystems,
    MarketPrices,
    MarketOrders,
    RegionIDs,
    Regions,
    Search,
    Stations,
    Structures,
    Systems,
    Types,
    Corporations,
    Alliances,
}

impl std::fmt::Display for CacheName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match &self {
            CacheName::Constellations => "constellations",
            CacheName::IndustrialSystems => "industrial_systems",
            CacheName::MarketPrices => "market_prices",
            CacheName::RegionIDs => "regions_ids",
            CacheName::Regions => "regions",
            CacheName::Search => "search",
            CacheName::Stations => "stations",
            CacheName::Structures => "structures",
            CacheName::Systems => "systems",
            CacheName::Types => "types",
            CacheName::Corporations => "corporations",
            CacheName::Alliances => "alliances",
            CacheName::MarketOrders => "market_orders",
        };
        write!(f, "{}", str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_from_cache_nonexistent_file() {
        let cache_directory = tempfile::tempdir().unwrap().into_path();
        let fs_cache = FSCache::new(cache_directory);

        let content = fs_cache
            .load_from_cache::<String>(CacheName::Constellations, Duration::seconds(60))
            .await
            .unwrap();

        assert_eq!(content, None);
    }

    #[tokio::test]
    async fn test_load_from_file_exist() {
        let cache_directory = tempfile::tempdir().unwrap().into_path();
        let fs_cache = FSCache::new(cache_directory);

        fs_cache
            .save_to_cache(CacheName::Alliances, vec!["test", "test2", "test3"])
            .unwrap();

        let content = fs_cache
            .load_from_cache::<Vec<String>>(CacheName::Alliances, Duration::seconds(60))
            .await
            .unwrap();

        assert_eq!(
            content,
            Some(vec![
                "test".to_string(),
                "test2".to_string(),
                "test3".to_string()
            ])
        );
    }

    #[tokio::test]
    async fn test_load_from_file_expired() {
        let cache_directory = tempfile::tempdir().unwrap().into_path();
        let fs_cache = FSCache::new(cache_directory);

        fs_cache
            .save_to_cache(CacheName::Alliances, vec!["test", "test2", "test3"])
            .unwrap();

        let content = fs_cache
            .load_from_cache::<Vec<String>>(CacheName::Alliances, Duration::microseconds(0))
            .await
            .unwrap();

        assert_eq!(content, None);
    }
}
