use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

use chrono::Duration;
use rfesi::groups::{
    AllianceInfo, CharacterOrder, CharacterPublicInfo, Constellation, CorporationPublicInfo,
    HistoryItem, IndustrialSystem, IndustryJob, LocationInfo, MarketOrder, PriceItem, Region,
    SearchResult, Skills, Station, Structure, System, Type,
};
use rfesi::prelude::Esi;

use crate::api::sde::{BlueprintActivityType, SDEBlueprint, Sde};
use crate::authentication::tokens::TokenHelper;
use crate::cache::CacheName::RegionIDs;
use crate::cache::{CacheName, FSCache, FSCacheReadError, FSCacheWriteError};
use crate::logging;

use self::cache::{
    Cache, CacheError, CacheableRetryableError, GlobalEntryCache, IdentifiedEntryCache, SingleCache,
};
use self::cache_keys::OrderType;
use self::loader::{APIError, CharacterBaseInfo, EsiLoader, KeyLoader, Loader};

use cache_keys::{MarketOrderKey, SearchKey};

pub mod cache;
pub mod cache_keys;
pub mod loader;

pub trait MarketTraits: MarketRegionHistoryLoader + MarketRegionOrdersLoader {}

pub trait EveRequester:
    StationLoader
    + StructureLoader
    + SystemLoader
    + ConstellationLoader
    + RegionLoader
    + TypeLoader
    + CorporationLoader
    + AllianceLoader
    + MarketOrderLoader
    + Searcher
    + MarketPricesLoader
    + IndustrialSystemsLoader
    + RegionIDsLoader
    + CharacterSkillsLoader
    + CharacterIndustryJobsLoader
    + CharacterMarketOrdersLoader
    + BlueprintsLoader
    + CharacterPublicInfoLoader
    + CharacterWalletLoader
    + CharacterLocationLoader
    + MarketRegionOrdersLoader
    + MarketRegionHistoryLoader
    + CharacterBasicInfoLoader
    + MarketTraits
{
}

#[async_trait]
pub trait CharacterBasicInfoLoader {
    async fn get_character_basic_info(&self) -> Result<CharacterBaseInfo, CacheError>;
}

#[async_trait]
pub trait StationLoader {
    async fn get_station(&self, id: i32) -> Result<Station, CacheError>;
}

#[async_trait]
pub trait StructureLoader {
    async fn get_structure(&self, id: i64) -> Result<Structure, CacheError>;
}

#[async_trait]
pub trait SystemLoader {
    async fn get_system(&self, id: i32) -> Result<System, CacheError>;
}

#[async_trait]
pub trait ConstellationLoader {
    async fn get_constellation(&self, id: i32) -> Result<Constellation, CacheError>;
}

#[async_trait]
pub trait RegionLoader {
    async fn get_region(&self, id: i32) -> Result<Region, CacheError>;
}

#[async_trait]
pub trait TypeLoader {
    async fn get_type(&self, id: i32) -> Result<Type, CacheError>;
}

#[async_trait]
pub trait CorporationLoader {
    async fn get_corporation(&self, id: i32) -> Result<CorporationPublicInfo, CacheError>;
}

#[async_trait]
pub trait AllianceLoader {
    async fn get_alliance(&self, id: i32) -> Result<AllianceInfo, CacheError>;
}

#[async_trait]
pub trait MarketOrderLoader {
    async fn get_market_orders(
        &self,
        region_id: i32,
        order_type: OrderType,
    ) -> Result<Vec<MarketOrder>, CacheError>;
}

#[async_trait]
pub trait Searcher {
    async fn search(
        &self,
        character_id: i32,
        categories: &str,
        search_str: &str,
        strict: Option<bool>,
    ) -> Result<SearchResult, CacheError>;
}

#[async_trait]
pub trait MarketPricesLoader {
    async fn get_market_prices(&self) -> Result<Vec<PriceItem>, CacheError>;
}

#[async_trait]
pub trait IndustrialSystemsLoader {
    async fn get_industry_systems(&self) -> Result<Vec<IndustrialSystem>, CacheError>;
}

#[async_trait]
pub trait RegionIDsLoader {
    async fn get_region_ids(&self) -> Result<Vec<i32>, CacheError>;
}

#[async_trait]
pub trait CharacterSkillsLoader {
    async fn get_character_skill(&self, id: i32) -> Result<Skills, CacheError>;
}

#[async_trait]
pub trait CharacterIndustryJobsLoader {
    async fn get_character_industry_jobs(&self, id: i32) -> Result<Vec<IndustryJob>, CacheError>;
}

#[async_trait]
pub trait CharacterMarketOrdersLoader {
    async fn get_character_orders(&self, id: i32) -> Result<Vec<CharacterOrder>, CacheError>;
}

#[async_trait]
pub trait BlueprintsLoader {
    async fn get_blueprint(
        &self,
        product_id: i32,
        activity: &BlueprintActivityType,
    ) -> Result<Option<(i32, SDEBlueprint)>, CacheError>;
    async fn get_blueprints(&self) -> Result<HashMap<i32, SDEBlueprint>, CacheError>;
}

#[async_trait]
pub trait CharacterPublicInfoLoader {
    async fn get_character_public_info(&self, id: i32) -> Result<CharacterPublicInfo, CacheError>;
}

#[async_trait]
pub trait CharacterWalletLoader {
    async fn get_character_wallet(&self, id: i32) -> Result<f64, CacheError>;
}

#[async_trait]
pub trait CharacterLocationLoader {
    async fn get_character_location(&self, id: i32) -> Result<LocationInfo, CacheError>;
}

#[async_trait]
pub trait MarketRegionOrdersLoader {
    async fn get_region_orders(
        &self,
        region_id: i32,
        order_type: Option<String>,
        page: Option<i32>,
        item_id: Option<i32>,
    ) -> Result<Vec<MarketOrder>, CacheError>;
}

#[async_trait]
pub trait MarketRegionHistoryLoader {
    async fn get_region_market_history(
        &self,
        region_id: i32,
        item_id: i32,
    ) -> Result<Vec<HistoryItem>, CacheError>;
}

pub enum CacheLevel {
    Disabled,
    Memory,
    Full(FSCache),
}

impl CacheLevel {
    pub fn from(value: &crate::configuration::cli::CacheLevel, fs_cache: FSCache) -> Self {
        match value {
            crate::configuration::cli::CacheLevel::Disabled => CacheLevel::Disabled,
            crate::configuration::cli::CacheLevel::Memory => CacheLevel::Memory,
            crate::configuration::cli::CacheLevel::Full => CacheLevel::Full(fs_cache),
        }
    }
}

pub struct EveCache {
    cache_level: CacheLevel,
    esi: Esi,
    token_helper: TokenHelper,
    sde: Sde,

    stations: Option<Cache<i32, Station>>,
    structures: Option<Cache<i64, Structure>>,
    systems: Option<Cache<i32, System>>,
    constellations: Option<Cache<i32, Constellation>>,
    regions: Option<Cache<i32, Region>>,
    types: Option<Cache<i32, Type>>,
    search: Option<Cache<SearchKey, SearchResult>>,
    market_prices: Option<SingleCache<Vec<PriceItem>>>,
    industrial_systems: Option<SingleCache<Vec<IndustrialSystem>>>,
    region_ids: Option<SingleCache<Vec<i32>>>,
    character_base_info: Option<SingleCache<CharacterBaseInfo>>,
    skills: Option<Cache<i32, Skills>>,
    corporations: Option<Cache<i32, CorporationPublicInfo>>,
    alliances: Option<Cache<i32, AllianceInfo>>,
    character_industry_jobs: Option<Cache<i32, Vec<IndustryJob>>>,
    character_orders: Option<Cache<i32, Vec<CharacterOrder>>>,
    market_orders: Option<Cache<MarketOrderKey, Vec<MarketOrder>>>,
}

impl EveCache {
    pub async fn new(
        esi: Esi,
        sde: Sde,
        token_helper: TokenHelper,
        cache_level: CacheLevel,
    ) -> Result<Self, FSCacheReadError> {
        match &cache_level {
            CacheLevel::Disabled => Ok(Self {
                cache_level,
                esi,
                sde,
                token_helper,
                stations: None,
                structures: None,
                systems: None,
                constellations: None,
                regions: None,
                types: None,
                search: None,
                market_prices: None,
                industrial_systems: None,
                region_ids: None,
                skills: None,
                corporations: None,
                alliances: None,
                character_industry_jobs: None,
                character_orders: None,
                market_orders: None,
                character_base_info: None,
            }),
            CacheLevel::Memory => Ok(Self {
                cache_level,
                esi,
                sde,
                token_helper,
                stations: Some(Cache::new()),
                structures: Some(Cache::new()),
                systems: Some(Cache::new()),
                constellations: Some(Cache::new()),
                regions: Some(Cache::new()),
                types: Some(Cache::new()),
                search: Some(Cache::new()),
                market_prices: Some(SingleCache::new()),
                industrial_systems: Some(SingleCache::new()),
                region_ids: Some(SingleCache::new()),
                character_base_info: Some(SingleCache::new()),
                skills: Some(Cache::new()),
                corporations: Some(Cache::new()),
                alliances: Some(Cache::new()),
                character_industry_jobs: Some(Cache::new()),
                character_orders: Some(Cache::new()),
                market_orders: Some(Cache::new()),
            }),
            CacheLevel::Full(fs_cache) => {
                let stations: Option<HashMap<i32, Station>> = fs_cache
                    .load_from_cache(CacheName::Stations, Duration::hours(2))
                    .await?;
                let structures: Option<HashMap<i64, Structure>> = fs_cache
                    .load_from_cache(CacheName::Structures, Duration::minutes(20))
                    .await?;
                let systems: Option<HashMap<i32, System>> = fs_cache
                    .load_from_cache(CacheName::Systems, Duration::hours(2))
                    .await?;
                let constellations: Option<HashMap<i32, Constellation>> = fs_cache
                    .load_from_cache(CacheName::Constellations, Duration::hours(2))
                    .await?;
                let regions: Option<HashMap<i32, Region>> = fs_cache
                    .load_from_cache(CacheName::Regions, Duration::hours(2))
                    .await?;
                let types: Option<HashMap<i32, Type>> = fs_cache
                    .load_from_cache(CacheName::Types, Duration::hours(2))
                    .await?;
                let corporations: Option<HashMap<i32, CorporationPublicInfo>> = fs_cache
                    .load_from_cache(CacheName::Corporations, Duration::minutes(20))
                    .await?;
                let alliances: Option<HashMap<i32, AllianceInfo>> = fs_cache
                    .load_from_cache(CacheName::Alliances, Duration::minutes(20))
                    .await?;
                let search: Option<HashMap<SearchKey, SearchResult>> = fs_cache
                    .load_from_cache(CacheName::Search, Duration::minutes(20))
                    .await?;
                let market_prices: Option<Vec<PriceItem>> = fs_cache
                    .load_from_cache(CacheName::MarketPrices, Duration::minutes(20))
                    .await?;
                let industrial_systems: Option<Vec<IndustrialSystem>> = fs_cache
                    .load_from_cache(CacheName::IndustrialSystems, Duration::minutes(20))
                    .await?;
                let region_ids: Option<Vec<i32>> = fs_cache
                    .load_from_cache(CacheName::RegionIDs, Duration::hours(2))
                    .await?;
                let market_orders: Option<HashMap<MarketOrderKey, Vec<MarketOrder>>> = fs_cache
                    .load_from_cache(CacheName::MarketOrders, Duration::hours(1))
                    .await?;

                Ok(Self {
                    cache_level,
                    esi,
                    sde,
                    token_helper,
                    stations: match stations {
                        None => Some(Cache::new()),
                        Some(stations) => Some(Cache::from(stations)),
                    },
                    structures: match structures {
                        None => Some(Cache::new()),
                        Some(structures) => Some(Cache::from(structures)),
                    },
                    systems: match systems {
                        None => Some(Cache::new()),
                        Some(systems) => Some(Cache::from(systems)),
                    },
                    constellations: match constellations {
                        None => Some(Cache::new()),
                        Some(constellations) => Some(Cache::from(constellations)),
                    },
                    regions: match regions {
                        None => Some(Cache::new()),
                        Some(regions) => Some(Cache::from(regions)),
                    },
                    types: match types {
                        None => Some(Cache::new()),
                        Some(types) => Some(Cache::from(types)),
                    },
                    search: match search {
                        None => Some(Cache::new()),
                        Some(search) => Some(Cache::from(search)),
                    },
                    market_prices: match market_prices {
                        None => Some(SingleCache::new()),
                        Some(market_prices) => Some(SingleCache::from(market_prices)),
                    },
                    industrial_systems: match industrial_systems {
                        None => Some(SingleCache::new()),
                        Some(industrial_systems) => Some(SingleCache::from(industrial_systems)),
                    },
                    region_ids: match region_ids {
                        None => Some(SingleCache::new()),
                        Some(region_ids) => Some(SingleCache::from(region_ids)),
                    },
                    corporations: match corporations {
                        None => Some(Cache::new()),
                        Some(corporations) => Some(Cache::from(corporations)),
                    },
                    alliances: match alliances {
                        None => Some(Cache::new()),
                        Some(alliances) => Some(Cache::from(alliances)),
                    },
                    skills: Some(Cache::new()),
                    character_industry_jobs: Some(Cache::new()),
                    character_orders: Some(Cache::new()),
                    character_base_info: Some(SingleCache::new()),
                    market_orders: match market_orders {
                        None => Some(Cache::new()),
                        Some(orders) => Some(Cache::from(orders)),
                    },
                })
            }
        }
    }

    pub async fn persist(&self) -> Result<(), FSCacheWriteError> {
        if let CacheLevel::Full(fs_cache) = &self.cache_level {
            fs_cache.save_to_cache(
                CacheName::Stations,
                self.stations
                    .as_ref()
                    .expect("Cannot have a FSCache without a memory cache.")
                    .memory_cache
                    .read()
                    .await
                    .clone(),
            )?;
            fs_cache.save_to_cache(
                CacheName::Structures,
                self.structures
                    .as_ref()
                    .expect("Cannot have a FSCache without a memory cache.")
                    .memory_cache
                    .read()
                    .await
                    .clone(),
            )?;
            fs_cache.save_to_cache(
                CacheName::Systems,
                self.systems
                    .as_ref()
                    .expect("Cannot have a FSCache without a memory cache.")
                    .memory_cache
                    .read()
                    .await
                    .clone(),
            )?;
            fs_cache.save_to_cache(
                CacheName::Constellations,
                self.constellations
                    .as_ref()
                    .expect("Cannot have a FSCache without a memory cache.")
                    .memory_cache
                    .read()
                    .await
                    .clone(),
            )?;
            fs_cache.save_to_cache(
                CacheName::Regions,
                self.regions
                    .as_ref()
                    .expect("Cannot have a FSCache without a memory cache.")
                    .memory_cache
                    .read()
                    .await
                    .clone(),
            )?;
            fs_cache.save_to_cache(
                CacheName::Types,
                self.types
                    .as_ref()
                    .expect("Cannot have a FSCache without a memory cache.")
                    .memory_cache
                    .read()
                    .await
                    .clone(),
            )?;
            fs_cache.save_to_cache(
                CacheName::Search,
                self.search
                    .as_ref()
                    .expect("Cannot have a FSCache without a memory cache.")
                    .memory_cache
                    .read()
                    .await
                    .clone(),
            )?;
            fs_cache.save_to_cache(
                CacheName::MarketPrices,
                self.market_prices
                    .as_ref()
                    .expect("Cannot have a FSCache without a memory cache.")
                    .memory_cache
                    .read()
                    .await
                    .clone(),
            )?;
            fs_cache.save_to_cache(
                CacheName::IndustrialSystems,
                self.industrial_systems
                    .as_ref()
                    .expect("Cannot have a FSCache without a memory cache.")
                    .memory_cache
                    .read()
                    .await
                    .clone(),
            )?;
            fs_cache.save_to_cache(
                RegionIDs,
                self.region_ids
                    .as_ref()
                    .expect("Cannot have a FSCache without a memory cache.")
                    .memory_cache
                    .read()
                    .await
                    .clone(),
            )?;
            fs_cache.save_to_cache(
                CacheName::Corporations,
                self.corporations
                    .as_ref()
                    .expect("Cannot have a FSCache without a memory cache.")
                    .memory_cache
                    .read()
                    .await
                    .clone(),
            )?;
            fs_cache.save_to_cache(
                CacheName::MarketOrders,
                self.market_orders
                    .as_ref()
                    .expect("Cannot have a FSCache without a memory cache.")
                    .memory_cache
                    .read()
                    .await
                    .clone(),
            )?;
        }
        Ok(())
    }
}

impl EveRequester for EveCache {}

impl MarketTraits for EveCache {}

#[async_trait]
impl CharacterBasicInfoLoader for EveCache {
    async fn get_character_basic_info(&self) -> Result<CharacterBaseInfo, CacheError> {
        query_cache(
            &self.character_base_info,
            EsiLoader::from(self),
            "character_base_info",
        )
        .await
    }
}

#[async_trait]
impl StationLoader for EveCache {
    async fn get_station(&self, id: i32) -> Result<Station, CacheError> {
        query_cache_with_id(&self.stations, id, EsiLoader::from(self), "station").await
    }
}

#[async_trait]
impl StructureLoader for EveCache {
    async fn get_structure(&self, id: i64) -> Result<Structure, CacheError> {
        query_cache_with_id(&self.structures, id, EsiLoader::from(self), "structure").await
    }
}

#[async_trait]
impl SystemLoader for EveCache {
    async fn get_system(&self, id: i32) -> Result<System, CacheError> {
        query_cache_with_id(&self.systems, id, EsiLoader::from(self), "system").await
    }
}

#[async_trait]
impl ConstellationLoader for EveCache {
    async fn get_constellation(&self, id: i32) -> Result<Constellation, CacheError> {
        query_cache_with_id(
            &self.constellations,
            id,
            EsiLoader::from(self),
            "constellation",
        )
        .await
    }
}

#[async_trait]
impl RegionLoader for EveCache {
    async fn get_region(&self, id: i32) -> Result<Region, CacheError> {
        query_cache_with_id(&self.regions, id, EsiLoader::from(self), "region").await
    }
}

#[async_trait]
impl TypeLoader for EveCache {
    async fn get_type(&self, id: i32) -> Result<Type, CacheError> {
        query_cache_with_id(&self.types, id, EsiLoader::from(self), "type").await
    }
}

#[async_trait]
impl CorporationLoader for EveCache {
    async fn get_corporation(&self, id: i32) -> Result<CorporationPublicInfo, CacheError> {
        query_cache_with_id(&self.corporations, id, EsiLoader::from(self), "corporation").await
    }
}

#[async_trait]
impl AllianceLoader for EveCache {
    async fn get_alliance(&self, id: i32) -> Result<AllianceInfo, CacheError> {
        query_cache_with_id(&self.alliances, id, EsiLoader::from(self), "alliance").await
    }
}

#[async_trait]
impl MarketOrderLoader for EveCache {
    async fn get_market_orders(
        &self,
        region_id: i32,
        order_type: OrderType,
    ) -> Result<Vec<MarketOrder>, CacheError> {
        query_cache_with_id(
            &self.market_orders,
            MarketOrderKey {
                region_id,
                order_type,
            },
            EsiLoader::from(self),
            "market_orders",
        )
        .await
    }
}

#[async_trait]
impl Searcher for EveCache {
    async fn search(
        &self,
        character_id: i32,
        categories: &str,
        search_str: &str,
        strict: Option<bool>,
    ) -> Result<SearchResult, CacheError> {
        let key = SearchKey {
            character_id,
            categories: categories.to_string(),
            search: search_str.to_string(),
            strict,
        };
        query_cache_with_id(&self.search, key, EsiLoader::from(self), "search").await
    }
}

#[async_trait]
impl MarketPricesLoader for EveCache {
    async fn get_market_prices(&self) -> Result<Vec<PriceItem>, CacheError> {
        query_cache(&self.market_prices, EsiLoader::from(self), "market_prices").await
    }
}

#[async_trait]
impl IndustrialSystemsLoader for EveCache {
    async fn get_industry_systems(&self) -> Result<Vec<IndustrialSystem>, CacheError> {
        query_cache(
            &self.industrial_systems,
            EsiLoader::from(self),
            "industrial_systems",
        )
        .await
    }
}

#[async_trait]
impl RegionIDsLoader for EveCache {
    async fn get_region_ids(&self) -> Result<Vec<i32>, CacheError> {
        query_cache(&self.region_ids, EsiLoader::from(self), "region_ids").await
    }
}

#[async_trait]
impl CharacterSkillsLoader for EveCache {
    async fn get_character_skill(&self, id: i32) -> Result<Skills, CacheError> {
        query_cache_with_id(&self.skills, id, EsiLoader::from(self), "skills").await
    }
}

#[async_trait]
impl CharacterIndustryJobsLoader for EveCache {
    async fn get_character_industry_jobs(&self, id: i32) -> Result<Vec<IndustryJob>, CacheError> {
        query_cache_with_id(
            &self.character_industry_jobs,
            id,
            EsiLoader::from(self),
            "character_industry_jobs",
        )
        .await
    }
}

#[async_trait]
impl CharacterMarketOrdersLoader for EveCache {
    async fn get_character_orders(&self, id: i32) -> Result<Vec<CharacterOrder>, CacheError> {
        query_cache_with_id(
            &self.character_orders,
            id,
            EsiLoader::from(self),
            "character_orders",
        )
        .await
    }
}

#[async_trait]
impl BlueprintsLoader for EveCache {
    async fn get_blueprints(&self) -> Result<HashMap<i32, SDEBlueprint>, CacheError> {
        Ok(self.sde.load_blueprints().await?)
    }

    async fn get_blueprint(
        &self,
        product_id: i32,
        activity: &BlueprintActivityType,
    ) -> Result<Option<(i32, SDEBlueprint)>, CacheError> {
        Ok(self.sde.load_blueprint(product_id, activity).await?)
    }
}

#[async_trait]
impl CharacterPublicInfoLoader for EveCache {
    async fn get_character_public_info(&self, id: i32) -> Result<CharacterPublicInfo, CacheError> {
        self.esi
            .group_character()
            .get_public_info(id)
            .await
            .map_err(|source| CacheError::Api {
                source: APIError {
                    description: "character_public_info".to_string(),
                    source,
                },
            })
    }
}

#[async_trait]
impl CharacterWalletLoader for EveCache {
    async fn get_character_wallet(&self, id: i32) -> Result<f64, CacheError> {
        self.esi
            .group_wallet()
            .get_wallet(id)
            .await
            .map_err(|source| CacheError::Api {
                source: APIError {
                    description: "character_wallet".to_string(),
                    source,
                },
            })
    }
}

#[async_trait]
impl CharacterLocationLoader for EveCache {
    async fn get_character_location(&self, id: i32) -> Result<LocationInfo, CacheError> {
        self.esi
            .group_location()
            .get_location(id)
            .await
            .map_err(|source| CacheError::Api {
                source: APIError {
                    description: "character_location".to_string(),
                    source,
                },
            })
    }
}

#[async_trait]
impl MarketRegionOrdersLoader for EveCache {
    async fn get_region_orders(
        &self,
        region_id: i32,
        order_type: Option<String>,
        page: Option<i32>,
        item_id: Option<i32>,
    ) -> Result<Vec<MarketOrder>, CacheError> {
        self.esi
            .group_market()
            .get_region_orders(region_id, order_type, page, item_id)
            .await
            .map_err(|source| CacheError::Api {
                source: APIError {
                    description: "region_orders".to_string(),
                    source,
                },
            })
    }
}

#[async_trait]
impl MarketRegionHistoryLoader for EveCache {
    async fn get_region_market_history(
        &self,
        region_id: i32,
        item_id: i32,
    ) -> Result<Vec<HistoryItem>, CacheError> {
        self.esi
            .group_market()
            .get_region_history(region_id, item_id)
            .await
            .map_err(|source| CacheError::Api {
                source: APIError {
                    description: "region_history".to_string(),
                    source,
                },
            })
    }
}

async fn query_cache<V: Clone, Err: CacheableRetryableError>(
    cache: &Option<impl GlobalEntryCache<V>>,
    loader: impl Loader<V, Err>,
    cache_name: &str,
) -> Result<V, CacheError> {
    match cache {
        None => Ok(loader
            .load()
            .await
            .map_err(Arc::new)
            .map_err(|source| CacheError::DataLoading { source })?),
        Some(cache) => {
            logging::trace!("Retrieving {}", cache_name);
            let val = cache.get_or_insert(&loader).await?;
            logging::trace!("Retrieved {}", cache_name);
            Ok(val)
        }
    }
}

async fn query_cache_with_id<
    K: Eq + PartialEq + Hash + Clone + Debug,
    V: Clone,
    Err: CacheableRetryableError,
>(
    cache: &Option<impl IdentifiedEntryCache<K, V>>,
    id: K,
    loader: impl KeyLoader<K, V, Err>,
    cache_name: &str,
) -> Result<V, CacheError> {
    match cache {
        None => Ok(loader
            .load(&id)
            .await
            .map_err(Arc::new)
            .map_err(|source| CacheError::DataLoading { source })?),
        Some(cache) => {
            logging::trace!("Retrieving {} (ID:{:?})", cache_name, id);
            let val = cache.get_or_insert(&id, &loader).await?;
            logging::trace!("Retrieved {} (ID:{:?})", cache_name, id);
            Ok(val)
        }
    }
}

#[cfg(test)]
pub mod mocks;
