use std::{collections::HashMap, time::Duration};

use crate::{
    authentication::tokens::{TokenError, TokenHelper},
    retry::{retry, RetryableError},
};
use futures_util::future::TryJoinAll;
use rfesi::{
    groups::{
        AllianceInfo, CharacterOrder, Constellation, CorporationPublicInfo, IndustrialSystem,
        IndustryJob, MarketOrder, PriceItem, Region, SearchResult, Skills, Station, Structure,
        System, Type,
    },
    prelude::{Esi, EsiError},
};
use thiserror::Error;
use tokio::try_join;

use crate::{
    cache::{CacheName, FSCacheWriteError},
    logging,
};

use super::{
    cache::CacheableRetryableError,
    cache_keys::{MarketOrderKey, SearchKey},
    CacheLevel, EveCache,
};

#[derive(Error, Debug)]
#[error("Could not perform request to ESI ({description}): {source}")]
pub struct APIError {
    pub description: String,
    pub source: EsiError,
}

impl CacheableRetryableError for APIError {}

impl RetryableError for APIError {
    fn retryable(&self) -> bool {
        if let EsiError::ReqwestError(e) = &self.source {
            return e.is_timeout();
        }
        false
    }
}

#[derive(Error, Debug)]
pub enum TokenInfoError {
    #[error("Access token not defined in ESI")]
    AccessTokenNotDefined,
    #[error("Could not decode token: {source}")]
    TokenDecodingFailed { source: TokenError },
    #[error("Could not extract Character ID from Access Token")]
    CharacterIDExtractFailed,
    #[error(transparent)]
    CharacterIDError {
        #[from]
        source: crate::authentication::tokens::CharacterIDError,
    },
}

impl CacheableRetryableError for TokenInfoError {}

impl RetryableError for TokenInfoError {
    fn retryable(&self) -> bool {
        false
    }
}

pub trait Loader<V, Err> {
    async fn load(&self) -> Result<V, Err>;
}

pub trait KeyLoader<K, V, Err: RetryableError> {
    async fn load(&self, key: &K) -> Result<V, Err>;
    async fn persist(&self, values: &HashMap<K, V>) -> Result<(), FSCacheWriteError>;
    async fn load_with_retry(&self, key: &K, nb_retry: u32) -> Result<V, Err> {
        retry(nb_retry, Duration::from_secs(1), async || {
            self.load(key).await
        })
        .await
    }
}

pub struct EsiLoader<'a> {
    esi: &'a Esi,
    token_helper: &'a TokenHelper,
    cache_level: &'a CacheLevel,
}

impl<'a> From<&'a EveCache> for EsiLoader<'a> {
    fn from(value: &'a EveCache) -> Self {
        Self {
            esi: &value.esi,
            cache_level: &value.cache_level,
            token_helper: &value.token_helper,
        }
    }
}

impl<'a> KeyLoader<i32, Station, APIError> for EsiLoader<'a> {
    async fn load(&self, id: &i32) -> Result<Station, APIError> {
        logging::trace!("Loading station with ID: {:?}", id);
        let mut error = None;
        for _ in 0..5 {
            let res = self.esi.group_universe().get_station(*id).await;
            match res {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    if let EsiError::ReqwestError(source) = &e {
                        if source.is_timeout() {
                            continue;
                        }
                    }
                    error = Some(APIError {
                        description: "get_station".to_string(),
                        source: e,
                    });
                    return Err(error.unwrap());
                }
            }
        }
        Err(error.unwrap())
    }

    async fn persist(&self, data: &HashMap<i32, Station>) -> Result<(), FSCacheWriteError> {
        if let CacheLevel::Full(fs_data) = self.cache_level {
            fs_data.save_to_cache(CacheName::Stations, data)?;
        }
        Ok(())
    }
}

impl<'a> KeyLoader<i64, Structure, APIError> for EsiLoader<'a> {
    async fn load(&self, id: &i64) -> Result<Structure, APIError> {
        logging::trace!("Loading structure with ID: {:?}", id);
        self.esi
            .group_universe()
            .get_structure(*id)
            .await
            .map_err(|source| APIError {
                description: "get_structure".to_string(),
                source,
            })
    }

    async fn persist(&self, data: &HashMap<i64, Structure>) -> Result<(), FSCacheWriteError> {
        if let CacheLevel::Full(fs_data) = self.cache_level {
            fs_data.save_to_cache(CacheName::Structures, data)?;
        }
        Ok(())
    }
}

impl<'a> KeyLoader<i32, System, APIError> for EsiLoader<'a> {
    async fn load(&self, id: &i32) -> Result<System, APIError> {
        logging::trace!("Loading system with ID: {:?}", id);
        self.esi
            .group_universe()
            .get_system(*id)
            .await
            .map_err(|source| APIError {
                description: "get_system".to_string(),
                source,
            })
    }

    async fn persist(&self, data: &HashMap<i32, System>) -> Result<(), FSCacheWriteError> {
        if let CacheLevel::Full(fs_data) = self.cache_level {
            fs_data.save_to_cache(CacheName::Systems, data)?;
        }
        Ok(())
    }
}

impl<'a> KeyLoader<i32, Constellation, APIError> for EsiLoader<'a> {
    async fn load(&self, id: &i32) -> Result<Constellation, APIError> {
        logging::trace!("Loading constellation with ID: {:?}", id);
        self.esi
            .group_universe()
            .get_constellation(*id)
            .await
            .map_err(|source| APIError {
                description: "get_constellation".to_string(),
                source,
            })
    }

    async fn persist(&self, data: &HashMap<i32, Constellation>) -> Result<(), FSCacheWriteError> {
        if let CacheLevel::Full(fs_data) = self.cache_level {
            fs_data.save_to_cache(CacheName::Constellations, data)?;
        }
        Ok(())
    }
}

impl<'a> KeyLoader<i32, Region, APIError> for EsiLoader<'a> {
    async fn load(&self, id: &i32) -> Result<Region, APIError> {
        logging::trace!("Loading region with ID: {:?}", id);
        self.esi
            .group_universe()
            .get_region(*id)
            .await
            .map_err(|source| APIError {
                description: "get_region".to_string(),
                source,
            })
    }

    async fn persist(&self, data: &HashMap<i32, Region>) -> Result<(), FSCacheWriteError> {
        if let CacheLevel::Full(fs_data) = self.cache_level {
            fs_data.save_to_cache(CacheName::Regions, data)?;
        }
        Ok(())
    }
}

impl<'a> KeyLoader<i32, Type, APIError> for EsiLoader<'a> {
    async fn load(&self, id: &i32) -> Result<Type, APIError> {
        logging::trace!("Loading type with ID: {:?}", id);
        self.esi
            .group_universe()
            .get_type(*id)
            .await
            .map_err(|source| APIError {
                description: format!("get_type: {}", id),
                source,
            })
    }

    async fn persist(&self, data: &HashMap<i32, Type>) -> Result<(), FSCacheWriteError> {
        if let CacheLevel::Full(fs_data) = self.cache_level {
            fs_data.save_to_cache(CacheName::Types, data)?;
        }
        Ok(())
    }
}

impl<'a> KeyLoader<i32, CorporationPublicInfo, APIError> for EsiLoader<'a> {
    async fn load(&self, id: &i32) -> Result<CorporationPublicInfo, APIError> {
        logging::trace!("Loading corporation with ID: {:?}", id);
        self.esi
            .group_corporation()
            .get_public_info(*id)
            .await
            .map_err(|source| APIError {
                description: "get_corporation".to_string(),
                source,
            })
    }

    async fn persist(
        &self,
        data: &HashMap<i32, CorporationPublicInfo>,
    ) -> Result<(), FSCacheWriteError> {
        if let CacheLevel::Full(fs_data) = self.cache_level {
            fs_data.save_to_cache(CacheName::Corporations, data)?;
        }
        Ok(())
    }
}

impl<'a> KeyLoader<i32, AllianceInfo, APIError> for EsiLoader<'a> {
    async fn load(&self, id: &i32) -> Result<AllianceInfo, APIError> {
        logging::trace!("Loading corporation with ID: {:?}", id);
        self.esi
            .group_alliance()
            .get_info(*id)
            .await
            .map_err(|source| APIError {
                description: "get_alliance".to_string(),
                source,
            })
    }

    async fn persist(&self, data: &HashMap<i32, AllianceInfo>) -> Result<(), FSCacheWriteError> {
        if let CacheLevel::Full(fs_data) = self.cache_level {
            fs_data.save_to_cache(CacheName::Alliances, data)?;
        }
        Ok(())
    }
}

impl<'a> KeyLoader<SearchKey, SearchResult, APIError> for EsiLoader<'a> {
    async fn load(&self, id: &SearchKey) -> Result<SearchResult, APIError> {
        logging::trace!("Search with ID: {:?}", id);
        self.esi
            .group_search()
            .search(
                id.character_id,
                id.categories.to_string(),
                id.search.to_string(),
                id.strict,
            )
            .await
            .map_err(|source| APIError {
                description: "search".to_string(),
                source,
            })
    }

    async fn persist(
        &self,
        data: &HashMap<SearchKey, SearchResult>,
    ) -> Result<(), FSCacheWriteError> {
        if let CacheLevel::Full(fs_data) = self.cache_level {
            fs_data.save_to_cache(CacheName::Search, data)?;
        }
        Ok(())
    }
}

impl<'a> Loader<Vec<PriceItem>, APIError> for EsiLoader<'a> {
    async fn load(&self) -> Result<Vec<PriceItem>, APIError> {
        logging::trace!("Load market prices");
        self.esi
            .group_market()
            .get_market_prices()
            .await
            .map_err(|source| APIError {
                description: "get_market_prices".to_string(),
                source,
            })
    }
}

impl<'a> Loader<Vec<IndustrialSystem>, APIError> for EsiLoader<'a> {
    async fn load(&self) -> Result<Vec<IndustrialSystem>, APIError> {
        logging::trace!("Load industrial systems");
        self.esi
            .group_industry()
            .get_industry_systems()
            .await
            .map_err(|source| APIError {
                description: "get_industry_systems".to_string(),
                source,
            })
    }
}

impl<'a> Loader<Vec<i32>, APIError> for EsiLoader<'a> {
    async fn load(&self) -> Result<Vec<i32>, APIError> {
        logging::trace!("Load region IDs");
        self.esi
            .group_universe()
            .get_region_ids()
            .await
            .map_err(|source| APIError {
                description: "get_region_ids".to_string(),
                source,
            })
    }
}

#[derive(Clone)]
pub struct CharacterBaseInfo {
    pub id: i32,
    pub name: String,
}

impl<'a> Loader<CharacterBaseInfo, TokenInfoError> for EsiLoader<'a> {
    async fn load(&self) -> Result<CharacterBaseInfo, TokenInfoError> {
        logging::trace!("Load character base info");
        let access_token = self
            .esi
            .access_token
            .as_ref()
            .ok_or(TokenInfoError::AccessTokenNotDefined)?;
        let token_data = self
            .token_helper
            .decode(access_token)
            .map_err(|source| TokenInfoError::TokenDecodingFailed { source })?;
        let character_id = self
            .token_helper
            .character_id(&token_data.claims)?
            .ok_or(TokenInfoError::CharacterIDExtractFailed)?;
        Ok(CharacterBaseInfo {
            id: character_id as i32,
            name: token_data.claims.name,
        })
    }
}

impl<'a> KeyLoader<i32, Skills, APIError> for EsiLoader<'a> {
    async fn load(&self, character_id: &i32) -> Result<Skills, APIError> {
        logging::trace!("Load skills for character ID: {:?}", character_id);
        self.esi
            .group_skills()
            .get_skills(*character_id)
            .await
            .map_err(|source| APIError {
                description: "get_character_skills".to_string(),
                source,
            })
    }

    async fn persist(&self, _: &HashMap<i32, Skills>) -> Result<(), FSCacheWriteError> {
        Ok(())
    }
}

impl<'a> KeyLoader<i32, Vec<IndustryJob>, APIError> for EsiLoader<'a> {
    async fn load(&self, character_id: &i32) -> Result<Vec<IndustryJob>, APIError> {
        logging::trace!("Load industry jobs for character ID: {:?}", character_id);
        self.esi
            .group_industry()
            .get_character_industry_jobs(*character_id, None)
            .await
            .map_err(|source| APIError {
                description: "get_character_industry_jobs".to_string(),
                source,
            })
    }

    async fn persist(&self, _: &HashMap<i32, Vec<IndustryJob>>) -> Result<(), FSCacheWriteError> {
        Ok(())
    }
}

impl<'a> KeyLoader<i32, Vec<CharacterOrder>, APIError> for EsiLoader<'a> {
    async fn load(&self, character_id: &i32) -> Result<Vec<CharacterOrder>, APIError> {
        logging::trace!("Load character orders for character ID: {:?}", character_id);
        self.esi
            .group_market()
            .get_character_orders(*character_id)
            .await
            .map_err(|source| APIError {
                description: "get_character_orders".to_string(),
                source,
            })
    }

    async fn persist(
        &self,
        _: &HashMap<i32, Vec<CharacterOrder>>,
    ) -> Result<(), FSCacheWriteError> {
        Ok(())
    }
}

impl<'a> KeyLoader<MarketOrderKey, Vec<MarketOrder>, APIError> for EsiLoader<'a> {
    async fn load(&self, id: &MarketOrderKey) -> Result<Vec<MarketOrder>, APIError> {
        let request_group_size = 20;
        let mut ext_index = 0;
        let mut all_orders = vec![];
        loop {
            let offset = ext_index * request_group_size;
            logging::trace!("Load market orders for: {:?} - Offset: {}", id, offset);
            let mut futures = vec![];
            for i in 1..21 {
                futures.push(async move {
                    let resp = self
                        .esi
                        .group_market()
                        .get_region_orders(
                            id.region_id,
                            Some(id.order_type.to_string().to_lowercase()),
                            Some(offset + i),
                            None,
                        )
                        .await;
                    match resp {
                        Ok(orders) => Ok(Some(orders)),
                        Err(e) => {
                            if let EsiError::InvalidStatusCode(code) = &e {
                                if 404 == *code {
                                    return Ok(None);
                                }
                            }
                            Err(APIError {
                                description: "market_orders".to_string(),
                                source: e,
                            })
                        }
                    }
                })
            }
            let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
            let loaded_orders = try_join!(all_futures)?.0;

            let mut end_reached = false;
            for orders in loaded_orders {
                match orders {
                    Some(orders) => {
                        for o in orders {
                            all_orders.push(o);
                        }
                    }
                    None => {
                        end_reached = true;
                    }
                }
            }

            if end_reached {
                break;
            } else {
                ext_index += 1;
            }
        }

        Ok(all_orders)
    }

    async fn persist(
        &self,
        _: &HashMap<MarketOrderKey, Vec<MarketOrder>>,
    ) -> Result<(), FSCacheWriteError> {
        Ok(())
    }
}

#[cfg(test)]
pub mod mocks {
    use std::collections::HashMap;

    use crate::{cache::FSCacheWriteError, retry::RetryableError};

    use super::{KeyLoader, Loader};

    pub struct MockGlobalLoader<Err: RetryableError + Clone> {
        pub result: Result<String, Err>,
    }

    impl<Err: RetryableError + Clone> Loader<String, Err> for MockGlobalLoader<Err> {
        async fn load(&self) -> Result<String, Err> {
            self.result.clone()
        }
    }

    pub struct MockKeyLoader<Err: RetryableError + Clone> {
        pub res: Option<Result<String, Err>>,
    }

    impl<Err: RetryableError + Clone> KeyLoader<u32, String, Err> for MockKeyLoader<Err> {
        async fn load(&self, key: &u32) -> Result<String, Err> {
            if let Some(res) = &self.res {
                return res.clone();
            }
            return Ok(format!("value_{}", key));
        }

        async fn persist(&self, _: &HashMap<u32, String>) -> Result<(), FSCacheWriteError> {
            Ok(())
        }
    }
}
