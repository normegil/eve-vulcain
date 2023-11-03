use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures_util::future::{try_join, try_join3, TryJoinAll};
use futures_util::TryFutureExt;
use rfesi::prelude::EsiError;
use thiserror::Error;
use tokio::try_join;

use crate::api::evecache::cache::CacheError;
use crate::api::evecache::cache_keys::OrderType;
use crate::api::evecache::{self, EveRequester};
use crate::api::sde::{BlueprintActivityType, SDEBlueprint};

use crate::filesystem::{FSData, FSFacilityType, FSReadError};
use crate::logging;
use crate::model::blueprint::{
    Activities, Blueprint, BlueprintInvention, BlueprintManufacturing, Materials,
};
use crate::model::character::{Alliance, Character, CharacterLocation, Corporation, Skills};
use crate::model::facility::markets::RegionOrders;
use crate::model::facility::playerstructure::PlayerStructureStats;
use crate::model::facility::{Facility, FacilityUsage};
use crate::model::industry::{IndustryType, Job};
use crate::model::items::{Item, TechLevel};
use crate::model::locations::{Constellation, CostIndexes, Region, SolarSystem};
use crate::model::markets::CharacterOrder;
use crate::model::prices::{ItemPrice, Prices};
use crate::model::skills::{Skill, TrainedSkill};

#[derive(Debug, Error)]
pub enum DataLoadError {
    #[error(transparent)]
    CacheError {
        #[from]
        source: evecache::cache::CacheError,
    },
}

#[derive(Debug, Error)]
pub enum CharacterLocationError {
    #[error(transparent)]
    DataLoadError(#[from] DataLoadError),
    #[error(transparent)]
    FacilityLoadingError(#[from] FacilityLoadingError),
    #[error(transparent)]
    SystemLoadingError(#[from] SystemLoadingError),
    #[error("Location type is not supported or unknown")]
    UnknownLocationType,
}

#[derive(Debug, Error)]
pub enum FacilityLoadingError {
    #[error(transparent)]
    DataLoadError(#[from] DataLoadError),
    #[error(transparent)]
    SystemLoadingError(#[from] SystemLoadingError),
    #[error("Wrong facility type loaded (expected:{expected};got:{got})")]
    LoadingFacilitiesMismatch { expected: String, got: String },
    #[error("Facility '{station_id}' could not be loaded: {source}")]
    LoadingFacilityError {
        station_id: i32,
        source: FSReadError,
    },
    #[error("Facilities could not be loaded: {source}")]
    LoadingFacilitiesError { source: FSReadError },
}

#[derive(Debug, Error)]
pub enum ItemLoadingError {
    #[error(transparent)]
    DataLoadError(#[from] DataLoadError),
    #[error("Items  could not be loaded: {source}")]
    LoadingRegisteredItemError { source: FSReadError },
}

#[derive(Debug, Error)]
pub enum SystemLoadingError {
    #[error(transparent)]
    DataLoadError(#[from] DataLoadError),
    #[error("System index not found for system '{solar_system_id}'")]
    SystemCostIndexNotFound { solar_system_id: i32 },
}

#[derive(Debug, Error)]
pub enum IndustryJobsLoadingError {
    #[error(transparent)]
    DataLoadError(#[from] DataLoadError),
    #[error("Job end date ({date_str}) could not be parsed: {source}")]
    ParsingJobEndDateError {
        source: chrono::ParseError,
        date_str: String,
    },
    #[error(transparent)]
    IndustryTypeDoesntExist {
        #[from]
        source: crate::model::industry::IndustryTypeNotFound,
    },
}

pub struct DataIntegrator {
    eve_cache: Arc<dyn EveRequester>,
    fs_data: FSData,
}

impl DataIntegrator {
    pub fn new(eve_cache: Arc<dyn EveRequester>, fs_data: FSData) -> Self {
        Self { eve_cache, fs_data }
    }

    pub fn fs(&self) -> &FSData {
        &self.fs_data
    }
    pub fn api(&self) -> Arc<dyn EveRequester> {
        self.eve_cache.clone()
    }
}

impl DataIntegrator {
    pub async fn load_character(&self) -> Result<Character, CharacterLocationError> {
        let character = self
            .eve_cache
            .get_character_basic_info()
            .await
            .map_err(|source| {
                CharacterLocationError::DataLoadError(DataLoadError::CacheError { source })
            })?;

        let (character_info, wallet) = try_join(
            self.eve_cache.get_character_public_info(character.id),
            self.eve_cache.get_character_wallet(character.id as i32),
        )
        .await
        .map_err(|source| {
            CharacterLocationError::DataLoadError(DataLoadError::CacheError { source })
        })?;

        let (location, corporation, skills) = try_join3(
            self.load_character_location(),
            self.load_corporation(character_info.corporation_id)
                .map_err(CharacterLocationError::DataLoadError),
            self.load_character_skills()
                .map_err(CharacterLocationError::DataLoadError),
        )
        .await?;

        Ok(Character::new(
            character.id,
            character.name.clone(),
            wallet,
            location,
            corporation,
            skills,
        ))
    }

    async fn load_character_skills(&self) -> Result<Skills, DataLoadError> {
        let character = self
            .eve_cache
            .get_character_basic_info()
            .await
            .map_err(|source| DataLoadError::CacheError { source })?;

        let skills = self
            .eve_cache
            .get_character_skill(character.id as i32)
            .await
            .map_err(|source| DataLoadError::CacheError { source })?;

        let mut futures = vec![];
        for skill in skills.skills {
            futures.push(async move {
                let skill_type = self.eve_cache.get_type(skill.skill_id).await?;
                Ok::<TrainedSkill, DataLoadError>(TrainedSkill::new(
                    skill.skill_id,
                    &skill_type.name,
                    skill.trained_skill_level,
                ))
            })
        }
        let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
        let skills = try_join!(all_futures)?.0;
        Ok(Skills { skills })
    }

    pub async fn load_station(&self, station_id: i32) -> Result<Facility, FacilityLoadingError> {
        let station_info = self
            .eve_cache
            .get_station(station_id)
            .await
            .map_err(|source| {
                FacilityLoadingError::DataLoadError(DataLoadError::CacheError { source })
            })?;
        let system = self.load_system(station_info.system_id).await?;
        let usages = match self
            .fs_data
            .load_facility(station_id as i64)
            .await
            .map_err(|source| FacilityLoadingError::LoadingFacilityError { station_id, source })?
        {
            None => None,
            Some(FSFacilityType::NPCStation(station)) => Some(station.usages),
            Some(FSFacilityType::PlayerStructure(_)) => {
                return Err(FacilityLoadingError::LoadingFacilitiesMismatch {
                    expected: "station".to_string(),
                    got: "structure".to_string(),
                });
            }
        };
        Ok(Facility::new_station(
            self.eve_cache.clone(),
            station_id,
            station_info.name.clone(),
            system,
            usages,
        ))
    }

    pub async fn load_structure(
        &self,
        structure_id: i64,
    ) -> Result<Facility, FacilityLoadingError> {
        let structure_info =
            self.eve_cache
                .get_structure(structure_id)
                .await
                .map_err(|source| {
                    FacilityLoadingError::DataLoadError(DataLoadError::CacheError { source })
                })?;
        let system = self.load_system(structure_info.solar_system_id).await?;
        let (usages, activities) =
            match self
                .fs_data
                .load_facility(structure_id)
                .await
                .map_err(|source| FacilityLoadingError::LoadingFacilityError {
                    station_id: structure_id as i32,
                    source,
                })? {
                None => (
                    Option::<Vec<FacilityUsage>>::None,
                    HashMap::<IndustryType, PlayerStructureStats>::new(),
                ),
                Some(FSFacilityType::PlayerStructure(structure)) => {
                    (Some(structure.usages), structure.activities)
                }
                Some(FSFacilityType::NPCStation(_)) => {
                    return Err(FacilityLoadingError::LoadingFacilitiesMismatch {
                        expected: "station".to_string(),
                        got: "structure".to_string(),
                    });
                }
            };
        Ok(Facility::new_structure(
            self.eve_cache.clone(),
            structure_id,
            structure_info.name.clone(),
            system,
            usages,
            activities,
        ))
    }

    pub async fn load_registered_facilities(&self) -> Result<Vec<Facility>, FacilityLoadingError> {
        let facilities = self
            .fs_data
            .load_facilities()
            .await
            .map_err(|source| FacilityLoadingError::LoadingFacilitiesError { source })?;

        let mut station_futures = vec![];
        for station in facilities.stations {
            station_futures.push(self.load_station(station.id))
        }
        let all_station_futures = station_futures.into_iter().collect::<TryJoinAll<_>>();

        let mut structure_futures = vec![];
        for structure in facilities.structures {
            structure_futures.push(self.load_structure(structure.id))
        }
        let all_structure_futures = structure_futures.into_iter().collect::<TryJoinAll<_>>();

        let (station_facilities, structure_facilities) =
            try_join(all_station_futures, all_structure_futures).await?;
        let mut facilities = vec![];
        for facility in station_facilities {
            facilities.push(facility);
        }
        for facility in structure_facilities {
            facilities.push(facility);
        }
        Ok(facilities)
    }

    pub async fn load_registered_items(&self) -> Result<Vec<Item>, ItemLoadingError> {
        let items = self
            .fs_data
            .load_items()
            .await
            .map_err(|source| ItemLoadingError::LoadingRegisteredItemError { source })?;

        let mut all_futures = vec![];
        for item_id in items.items {
            all_futures.push(self.load_item(item_id))
        }
        let all_futures = all_futures.into_iter().collect::<TryJoinAll<_>>();
        let all_items = try_join!(all_futures)?.0;
        Ok(all_items)
    }

    pub async fn load_item(&self, id: i32) -> Result<Item, DataLoadError> {
        let loaded = self.eve_cache.get_type(id).await?;

        let mut tech_level = TechLevel::One;
        let blueprint = self
            .eve_cache
            .get_blueprint(loaded.type_id, &BlueprintActivityType::Manufacturing)
            .await?;
        if let Some((bp_id, _)) = blueprint {
            let invention_blueprint = self
                .eve_cache
                .get_blueprint(bp_id, &BlueprintActivityType::Invention)
                .await?;
            if invention_blueprint.is_some() {
                tech_level = TechLevel::Two
            }
        }

        Ok(Item::new(id, &loaded.name, loaded.volume, tech_level))
    }

    pub async fn load_system(&self, system_id: i32) -> Result<SolarSystem, SystemLoadingError> {
        let system_cost_indices = self
            .eve_cache
            .get_industry_systems()
            .await
            .map_err(|source| DataLoadError::CacheError { source })?;
        let mut found_index = None;
        for system_cost_index in system_cost_indices {
            if system_id == system_cost_index.solar_system_id {
                found_index = Some(system_cost_index.cost_indices)
            }
        }
        let found_indexes = found_index.ok_or(SystemLoadingError::SystemCostIndexNotFound {
            solar_system_id: system_id,
        })?;
        let cost_indexes = CostIndexes::from(&found_indexes);

        let system = self
            .eve_cache
            .get_system(system_id)
            .await
            .map_err(|source| DataLoadError::CacheError { source })?;
        let constellation = self.load_constellation(system.constellation_id).await?;
        Ok(SolarSystem::new(
            system_id,
            system.name,
            system.security_status,
            system.stations.unwrap_or(vec![]),
            constellation,
            cost_indexes,
        ))
    }

    pub async fn load_constellation(
        &self,
        constellation_id: i32,
    ) -> Result<Constellation, DataLoadError> {
        let constellation = self.eve_cache.get_constellation(constellation_id).await?;
        let region = self.load_region(constellation.region_id).await?;
        Ok(Constellation::new(
            constellation_id,
            constellation.name,
            constellation.systems,
            region,
        ))
    }

    pub async fn load_region(&self, region_id: i32) -> Result<Region, DataLoadError> {
        let region = self.eve_cache.get_region(region_id).await?;
        Ok(Region::new(region_id, &region.name, region.constellations))
    }

    pub async fn load_corporation(
        &self,
        corporation_id: i32,
    ) -> Result<Corporation, DataLoadError> {
        let info = self.eve_cache.get_corporation(corporation_id).await?;
        let alliance = match info.alliance_id {
            None => None,
            Some(alliance_id) => {
                let alliance = self.eve_cache.get_alliance(alliance_id).await?;
                Some(Alliance::new(alliance_id, alliance.name))
            }
        };
        Ok(Corporation::new(corporation_id, info.name, alliance))
    }

    pub async fn load_all_items_with_blueprint(
        &self,
        industry_type: IndustryType,
    ) -> Result<Vec<Item>, DataLoadError> {
        let bps = self.load_blueprints(industry_type).await?;
        logging::debug!("All blueprints loaded for {}", &industry_type);
        let mut items = vec![];

        for blueprint in bps {
            match industry_type {
                IndustryType::Manufacturing => {
                    let products = blueprint
                        .activities
                        .manufacturing
                        .expect(
                            "Filtered based on industry type (Manufacturing) - Should not failed",
                        )
                        .products;
                    for product in products {
                        items.push(product.item.clone())
                    }
                }
                IndustryType::Invention => {
                    let products = blueprint
                        .activities
                        .invention
                        .expect("Filtered based on industry type (Invention) - Should not failed")
                        .products;
                    for product in products {
                        items.push(product.item.clone())
                    }
                }
                IndustryType::ResearchTimeEfficiency => todo!(),
                IndustryType::ResearchMaterialEfficiency => todo!(),
                IndustryType::Copying => todo!(),
                IndustryType::Reaction => todo!(),
            }
        }

        Ok(items)
    }

    pub async fn load_blueprints(
        &self,
        industry_type: IndustryType,
    ) -> Result<Vec<Blueprint>, DataLoadError> {
        let blueprints = self.eve_cache.get_blueprints().await?;

        let found_blueprints: Vec<(i32, SDEBlueprint)> = blueprints
            .into_iter()
            .filter(|(_, b)| is_searched_blueprint(b, industry_type))
            .collect();

        let mut futures = vec![];
        for (blueprint_id, found_blueprint) in found_blueprints {
            futures.push(async move {
                let res = self.load_blueprint(blueprint_id, found_blueprint).await;
                match res {
                    Ok(bp) => Ok(Some(bp)),
                    Err(e) => {
                        match &e {
                            DataLoadError::CacheError { source } => {
                                if let CacheError::Api { source } = &source {
                                    if source.description.contains("get_type") {
                                        if let EsiError::InvalidStatusCode(code) = &source.source {
                                            if &404 == code {
                                                logging::warning!(
                                                    "Ignored blueprint (ID:{}): {}",
                                                    blueprint_id,
                                                    e
                                                );
                                                return Ok(None);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e)
                    }
                }
            })
        }
        let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
        let blueprints = try_join!(all_futures)?.0;
        let blueprints = blueprints.into_iter().flatten().collect();
        Ok(blueprints)
    }

    pub async fn load_item_blueprints(
        &self,
        output_item_id: i32,
        industry_type: IndustryType,
    ) -> Result<Vec<Blueprint>, DataLoadError> {
        let blueprints = self.eve_cache.get_blueprints().await?;

        let found_blueprints: Vec<(i32, SDEBlueprint)> = blueprints
            .into_iter()
            .filter(|(_, b)| is_searched_item_blueprint(output_item_id, b, industry_type))
            .collect();

        let mut futures = vec![];
        for (blueprint_id, found_blueprint) in found_blueprints {
            futures.push(self.load_blueprint(blueprint_id, found_blueprint))
        }
        let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
        let blueprints = try_join!(all_futures)?.0;
        Ok(blueprints)
    }

    pub async fn search_items(
        &self,
        from: LoadFrom,
        strict: bool,
    ) -> Result<Vec<Item>, DataLoadError> {
        match from {
            LoadFrom::Name(name) => {
                let character = self
                    .eve_cache
                    .get_character_basic_info()
                    .await
                    .map_err(|source| DataLoadError::CacheError { source })?;

                let search_result = self
                    .eve_cache
                    .search(character.id as i32, "inventory_type", &name, Some(strict))
                    .await?;
                match search_result.inventory_type {
                    None => Ok(vec![]),
                    Some(item_ids) => {
                        let mut futures = vec![];
                        for item_id in item_ids {
                            futures.push(self.load_item(item_id));
                        }
                        let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
                        let items = try_join!(all_futures)?.0;
                        Ok(items)
                    }
                }
            }
        }
    }

    pub async fn search_structure(
        &self,
        name: &str,
    ) -> Result<Vec<Facility>, FacilityLoadingError> {
        let character = self
            .eve_cache
            .get_character_basic_info()
            .await
            .map_err(|source| DataLoadError::CacheError { source })?;

        let search_result = self
            .eve_cache
            .search(character.id as i32, "structure", name, Some(false))
            .await
            .map_err(|source| DataLoadError::CacheError { source })?;
        match search_result.structure {
            None => Ok(vec![]),
            Some(structure_ids) => {
                let mut futures = vec![];
                for structure_id in structure_ids {
                    futures.push(self.load_structure(structure_id as i64));
                }
                let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
                let items = try_join!(all_futures)?.0;
                Ok(items)
            }
        }
    }

    pub async fn load_prices(&self) -> Result<Prices, DataLoadError> {
        let item_prices = self.eve_cache.get_market_prices().await?;
        let mut prices = HashMap::new();
        for item_price in item_prices {
            prices.insert(
                item_price.type_id,
                ItemPrice::new(item_price.adjusted_price, item_price.average_price),
            );
        }
        Ok(Prices::new(prices))
    }

    pub async fn load_all_regions(&self) -> Result<Vec<Region>, DataLoadError> {
        let region_ids = self.eve_cache.get_region_ids().await?;

        let mut futures = vec![];
        for region_id in region_ids {
            futures.push(self.load_region(region_id));
        }
        let all_operations = futures.into_iter().collect::<TryJoinAll<_>>();
        let regions = try_join!(all_operations)?.0;
        Ok(regions)
    }

    pub async fn load_character_industry_jobs(&self) -> Result<Vec<Job>, IndustryJobsLoadingError> {
        let character = self
            .eve_cache
            .get_character_basic_info()
            .await
            .map_err(|source| DataLoadError::CacheError { source })?;

        let industry_jobs = self
            .eve_cache
            .get_character_industry_jobs(character.id as i32)
            .await
            .map_err(|source| DataLoadError::CacheError { source })?;

        let mut futures = vec![];
        for industry_job in industry_jobs {
            futures.push(async move {
                let industry_type =
                    IndustryType::try_from(industry_job.activity_id).map_err(|source| {
                        IndustryJobsLoadingError::IndustryTypeDoesntExist { source }
                    })?;
                let item: Option<Item> = if let Some(product_id) = industry_job.product_type_id {
                    Some(self.load_item(product_id).await?)
                } else {
                    None
                };
                let end_date =
                    DateTime::parse_from_rfc3339(&industry_job.end_date).map_err(|source| {
                        IndustryJobsLoadingError::ParsingJobEndDateError {
                            source,
                            date_str: industry_job.end_date,
                        }
                    })?;
                let end_date = end_date.with_timezone(&Utc);
                Ok::<Job, IndustryJobsLoadingError>(Job::new(
                    industry_type,
                    item,
                    industry_job.runs,
                    end_date,
                ))
            })
        }
        let all_operations = futures.into_iter().collect::<TryJoinAll<_>>();
        let jobs = try_join!(all_operations)?.0;
        Ok(jobs)
    }

    pub async fn load_character_orders(&self) -> Result<Vec<CharacterOrder>, DataLoadError> {
        let character = self
            .eve_cache
            .get_character_basic_info()
            .await
            .map_err(|source| DataLoadError::CacheError { source })?;

        let orders = self
            .eve_cache
            .get_character_orders(character.id as i32)
            .await?;

        let mut futures = vec![];
        for order in orders {
            futures.push(async move {
                let item = self.load_item(order.type_id).await?;
                let order_type = if let Some(is_buy_order) = order.is_buy_order {
                    if is_buy_order {
                        OrderType::Buy
                    } else {
                        OrderType::Sell
                    }
                } else {
                    OrderType::Sell
                };
                Ok::<CharacterOrder, DataLoadError>(CharacterOrder {
                    item,
                    order_type,
                    price: order.price,
                    volume_remain: order.volume_remain,
                    volume_total: order.volume_total,
                })
            })
        }
        let all_operations = futures.into_iter().collect::<TryJoinAll<_>>();
        let orders = try_join!(all_operations)?.0;
        Ok(orders)
    }

    pub async fn load_market_orders(
        &self,
        region_id: i32,
        order_type: OrderType,
    ) -> Result<RegionOrders, DataLoadError> {
        let orders = self
            .eve_cache
            .get_market_orders(region_id, order_type)
            .await?;
        let region = self.load_region(region_id).await?;
        Ok(RegionOrders { region, orders })
    }
}

impl DataIntegrator {
    async fn load_character_location(&self) -> Result<CharacterLocation, CharacterLocationError> {
        let character = self
            .eve_cache
            .get_character_basic_info()
            .await
            .map_err(|source| DataLoadError::CacheError { source })?;

        let location_info = self
            .eve_cache
            .get_character_location(character.id)
            .await
            .map_err(|source| {
                CharacterLocationError::DataLoadError(DataLoadError::CacheError { source })
            })?;

        let mut facility = None;
        let mut system = None;
        if let Some(station_id) = location_info.station_id {
            facility = Some(self.load_station(station_id).await?);
        } else if let Some(structure_id) = location_info.structure_id {
            facility = Some(self.load_structure(structure_id).await?);
        } else {
            system = Some(self.load_system(location_info.solar_system_id).await?);
        }

        if let Some(facility) = facility {
            Ok(CharacterLocation::Facility(facility))
        } else if let Some(system) = system {
            Ok(CharacterLocation::Space(system))
        } else {
            Err(CharacterLocationError::UnknownLocationType)
        }
    }

    async fn load_blueprint(
        &self,
        blueprint_id: i32,
        blueprint: SDEBlueprint,
    ) -> Result<Blueprint, DataLoadError> {
        let (manufacturing, invention) = try_join(
            self.load_blueprint_manufacturing(blueprint_id, &blueprint),
            self.load_blueprint_invention(blueprint_id, &blueprint),
        )
        .await?;

        Ok(Blueprint {
            id: blueprint_id,
            activities: Activities {
                manufacturing,
                invention,
            },
        })
    }

    async fn load_blueprint_manufacturing(
        &self,
        blueprint_id: i32,
        blueprint: &SDEBlueprint,
    ) -> Result<Option<BlueprintManufacturing>, DataLoadError> {
        if let Some(blueprint_manufacturing) = &blueprint.activities.manufacturing {
            let mut material_futures = vec![];
            if let Some(materials) = &blueprint_manufacturing.materials {
                for material in materials {
                    material_futures.push(async {
                        let item = self.load_item(material.type_id).await?;
                        Ok::<crate::model::blueprint::MultipleItems, DataLoadError>(
                            crate::model::blueprint::MultipleItems {
                                quantity: material.quantity,
                                item,
                            },
                        )
                    });
                }
            }
            let material_futures = material_futures.into_iter().collect::<TryJoinAll<_>>();
            let mut product_futures = vec![];
            if let Some(products) = &blueprint_manufacturing.products {
                for product in products {
                    product_futures.push(async {
                        let item = self.load_item(product.type_id).await?;
                        Ok::<crate::model::blueprint::MultipleItems, DataLoadError>(
                            crate::model::blueprint::MultipleItems {
                                quantity: product.quantity,
                                item,
                            },
                        )
                    });
                }
            }
            let product_futures = product_futures.into_iter().collect::<TryJoinAll<_>>();

            let invention_blueprint_future =
                self.load_item_blueprints(blueprint_id, IndustryType::Invention);

            let (materials, products, invention_bps) = try_join3(
                material_futures,
                product_futures,
                invention_blueprint_future,
            )
            .await?;

            let mut bps = vec![];
            for bp in invention_bps {
                if let Some(invent) = bp.activities.invention {
                    bps.push(invent.clone());
                }
            }

            return Ok(Some(BlueprintManufacturing {
                blueprint_id,
                materials: Materials::new(materials),
                products,
                material_efficiency: 0,
                time_efficiency: 0,
                time: blueprint_manufacturing.time,
                invention_blueprint: bps,
            }));
        }
        Ok(None)
    }

    async fn load_blueprint_invention(
        &self,
        blueprint_id: i32,
        blueprint: &SDEBlueprint,
    ) -> Result<Option<BlueprintInvention>, DataLoadError> {
        if let Some(blueprint_invention) = &blueprint.activities.invention {
            let mut material_futures = vec![];
            if let Some(materials) = &blueprint_invention.materials {
                for material in materials {
                    material_futures.push(async {
                        let item = self.load_item(material.type_id).await?;
                        Ok::<crate::model::blueprint::MultipleItems, DataLoadError>(
                            crate::model::blueprint::MultipleItems {
                                quantity: material.quantity,
                                item,
                            },
                        )
                    });
                }
            }
            let material_futures = material_futures.into_iter().collect::<TryJoinAll<_>>();
            let mut product_futures = vec![];
            if let Some(products) = &blueprint_invention.products {
                for product in products {
                    product_futures.push(async {
                        let item = self.load_item(product.type_id).await?;
                        Ok::<crate::model::blueprint::ProbableMultipleItems, DataLoadError>(
                            crate::model::blueprint::ProbableMultipleItems {
                                quantity: product.quantity,
                                item,
                                base_probability: product.probability,
                            },
                        )
                    });
                }
            }
            let product_futures = product_futures.into_iter().collect::<TryJoinAll<_>>();

            let (materials, products) = try_join(material_futures, product_futures).await?;

            let mut skills = vec![];
            if let Some(blueprint_skills) = &blueprint_invention.skills {
                for skill in blueprint_skills {
                    skills.push(self.eve_cache.get_type(skill.type_id));
                }
            }
            let skills = skills.into_iter().collect::<TryJoinAll<_>>();
            let skill_types = try_join!(skills)?.0;

            let mut skills = vec![];
            for t in skill_types {
                skills.push(Skill::new(t.type_id, &t.name));
            }

            return Ok(Some(BlueprintInvention {
                blueprint_id,
                materials: Materials::new(materials),
                products,
                skills,
                time: blueprint_invention.time,
            }));
        }
        Ok(None)
    }
}

fn is_searched_item_blueprint(
    output_item_id: i32,
    b: &SDEBlueprint,
    industry_type: IndustryType,
) -> bool {
    match industry_type {
        IndustryType::Manufacturing => {
            let blueprint_manufacturing = &b.activities.manufacturing;
            if let Some(blueprint_manufacturing) = blueprint_manufacturing {
                if let Some(products) = &blueprint_manufacturing.products {
                    for product in products {
                        if product.type_id == output_item_id {
                            return true;
                        }
                    }
                }
            }
        }
        IndustryType::Copying => {
            todo!()
        }
        IndustryType::Invention => {
            if let Some(blueprint_invention) = &b.activities.invention {
                if let Some(products) = &blueprint_invention.products {
                    for product in products {
                        if product.type_id == output_item_id {
                            return true;
                        }
                    }
                }
            }
        }
        IndustryType::Reaction => {
            todo!()
        }
        IndustryType::ResearchTimeEfficiency => todo!(),
        IndustryType::ResearchMaterialEfficiency => todo!(),
    }
    false
}

fn is_searched_blueprint(b: &SDEBlueprint, industry_type: IndustryType) -> bool {
    match industry_type {
        IndustryType::Manufacturing => {
            let blueprint_manufacturing = &b.activities.manufacturing;
            if let Some(blueprint_manufacturing) = blueprint_manufacturing {
                if blueprint_manufacturing.products.is_some() {
                    return true;
                }
            }
        }
        IndustryType::Copying => {
            todo!()
        }
        IndustryType::Invention => {
            if let Some(blueprint_invention) = &b.activities.invention {
                if blueprint_invention.products.is_some() {
                    return true;
                }
            }
        }
        IndustryType::Reaction => {
            todo!()
        }
        IndustryType::ResearchTimeEfficiency => todo!(),
        IndustryType::ResearchMaterialEfficiency => todo!(),
    }
    false
}

#[derive(Debug, Clone)]
pub enum LoadFrom {
    Name(String),
}

#[cfg(test)]
pub mod testutils {
    use std::sync::Arc;

    use crate::{
        api::evecache::{mocks::MockRequester, EveRequester},
        filesystem::testutils::{create_test_fs_data, prewrite_facilities, prewrite_items},
    };

    use super::DataIntegrator;

    pub fn create_test_data_integrator() -> (DataIntegrator, Arc<dyn EveRequester>) {
        let requester = MockRequester::default();
        let (fs_data, data_directory) = create_test_fs_data();

        prewrite_facilities(
            &data_directory,
            r#"{
            "stations": [
              {
                "id": 8,
                "usages": [
                  "Market",
                  "Industry"
                ]
              }
            ],
            "structures": [
                {
                  "id": 15,
                  "usages": [
                    "Industry"
                  ],
                  "activities": {
                    "Manufacturing": {
                      "tax_rate": 0.341,
                      "job_duration_modifier": 3.0,
                      "job_cost_modifier": 5.0,
                      "material_consumption_modifier": 2.5
                    },
                    "Invention": {
                      "tax_rate": 0.32,
                      "job_duration_modifier": 3.5,
                      "job_cost_modifier": 3.0,
                      "material_consumption_modifier": 3.5
                    }
                  }
                }
              ]
          }"#,
        );

        prewrite_items(
            &data_directory,
            r#"{
                "items": [
                  18,
                  19,
                  20
                ]
              }"#,
        );

        let requester = Arc::new(requester);
        (DataIntegrator::new(requester.clone(), fs_data), requester)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use chrono::{TimeZone, Utc};

    use crate::{
        api::evecache::cache_keys::OrderType,
        integration::{testutils::create_test_data_integrator, LoadFrom},
        model::{
            blueprint::{
                Activities, Blueprint, BlueprintInvention, BlueprintManufacturing, Materials,
                MultipleItems, ProbableMultipleItems,
            },
            character::{Alliance, Character, CharacterLocation, Corporation, Skills},
            common::Identified,
            facility::{playerstructure::PlayerStructureStats, Facility, FacilityUsage},
            industry::{IndustryType, Job},
            items::{Item, TechLevel},
            locations::{Constellation, CostIndexes, Region, SolarSystem},
            markets::CharacterOrder,
            prices::{ItemPrice, Prices},
            skills::TrainedSkill,
        },
    };

    #[tokio::test]
    pub async fn test_load_character() {
        let (data_integrator, requester) = create_test_data_integrator();
        let got = data_integrator.load_character().await.unwrap();
        let expected = Character {
            id: 1,
            name: "Test Name".to_string(),
            isk: 12345.67,
            location: CharacterLocation::Facility(Facility::new_station(
                requester.clone(),
                8,
                "Test Station Name".to_string(),
                SolarSystem::new(
                    9,
                    "Test Solar System".to_string(),
                    0.1234,
                    vec![8],
                    Constellation::new(
                        10,
                        "Test Constellation".to_string(),
                        vec![9],
                        Region::new(11, "Test Region", vec![10]),
                    ),
                    CostIndexes {
                        manufacturing: 0.456,
                        invention: 0.789,
                    },
                ),
                Some(vec![FacilityUsage::Market, FacilityUsage::Industry]),
            )),
            corporation: Corporation {
                id: 2,
                name: "Test Corp".to_string(),
                alliance: Some(Alliance {
                    id: 3,
                    name: "Test Alliance".to_string(),
                }),
            },
            skills: Skills {
                skills: vec![
                    TrainedSkill::new(4, "Test Skill n4", 2),
                    TrainedSkill::new(5, "Test Skill n5", 5),
                    TrainedSkill::new(6, "Test Skill n6", 1),
                    TrainedSkill::new(7, "Test Skill n7", 0),
                ],
            },
        };

        // Better inequality targeting
        assert_eq!(got.id, expected.id);
        assert_eq!(got.name, expected.name);
        assert_eq!(got.isk, expected.isk);
        assert_eq!(got.location, expected.location);
        assert_eq!(got.corporation, expected.corporation);
        assert_eq!(got.skills, expected.skills);

        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_load_character_skills() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator.load_character_skills().await.unwrap();
        let expected = Skills {
            skills: vec![
                TrainedSkill::new(4, "Test Skill n4", 2),
                TrainedSkill::new(5, "Test Skill n5", 5),
                TrainedSkill::new(6, "Test Skill n6", 1),
                TrainedSkill::new(7, "Test Skill n7", 0),
            ],
        };

        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_load_station() {
        let (data_integrator, requester) = create_test_data_integrator();
        let got = data_integrator.load_station(8).await.unwrap();
        let expected = Facility::new_station(
            requester.clone(),
            8,
            "Test Station Name".to_string(),
            SolarSystem::new(
                9,
                "Test Solar System".to_string(),
                0.1234,
                vec![8],
                Constellation::new(
                    10,
                    "Test Constellation".to_string(),
                    vec![9],
                    Region::new(11, "Test Region", vec![10]),
                ),
                CostIndexes {
                    manufacturing: 0.456,
                    invention: 0.789,
                },
            ),
            Some(vec![FacilityUsage::Market, FacilityUsage::Industry]),
        );
        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_load_structure() {
        let (data_integrator, requester) = create_test_data_integrator();
        let got = data_integrator.load_structure(15).await.unwrap();

        let mut activites = HashMap::new();
        activites.insert(
            IndustryType::Manufacturing,
            PlayerStructureStats {
                tax_rate: 0.341,
                job_duration_modifier: Some(3.0),
                job_cost_modifier: Some(5.0),
                material_consumption_modifier: Some(2.5),
            },
        );
        activites.insert(
            IndustryType::Invention,
            PlayerStructureStats {
                tax_rate: 0.32,
                job_duration_modifier: Some(3.5),
                job_cost_modifier: Some(3.0),
                material_consumption_modifier: Some(3.5),
            },
        );
        let expected = Facility::new_structure(
            requester.clone(),
            15,
            "Test Structure".to_string(),
            SolarSystem::new(
                9,
                "Test Solar System".to_string(),
                0.1234,
                vec![8],
                Constellation::new(
                    10,
                    "Test Constellation".to_string(),
                    vec![9],
                    Region::new(11, "Test Region", vec![10]),
                ),
                CostIndexes {
                    manufacturing: 0.456,
                    invention: 0.789,
                },
            ),
            Some(vec![FacilityUsage::Industry]),
            activites,
        );
        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_load_registered_facilities() {
        let (data_integrator, requester) = create_test_data_integrator();
        let got = data_integrator.load_registered_facilities().await.unwrap();

        let mut activites = HashMap::new();
        activites.insert(
            IndustryType::Manufacturing,
            PlayerStructureStats {
                tax_rate: 0.341,
                job_duration_modifier: Some(3.0),
                job_cost_modifier: Some(5.0),
                material_consumption_modifier: Some(2.5),
            },
        );
        activites.insert(
            IndustryType::Invention,
            PlayerStructureStats {
                tax_rate: 0.32,
                job_duration_modifier: Some(3.5),
                job_cost_modifier: Some(3.0),
                material_consumption_modifier: Some(3.5),
            },
        );
        let expected = vec![
            Facility::new_station(
                requester.clone(),
                8,
                "Test Station Name".to_string(),
                SolarSystem::new(
                    9,
                    "Test Solar System".to_string(),
                    0.1234,
                    vec![8],
                    Constellation::new(
                        10,
                        "Test Constellation".to_string(),
                        vec![9],
                        Region::new(11, "Test Region", vec![10]),
                    ),
                    CostIndexes {
                        manufacturing: 0.456,
                        invention: 0.789,
                    },
                ),
                Some(vec![FacilityUsage::Market, FacilityUsage::Industry]),
            ),
            Facility::new_structure(
                requester.clone(),
                15,
                "Test Structure".to_string(),
                SolarSystem::new(
                    9,
                    "Test Solar System".to_string(),
                    0.1234,
                    vec![8],
                    Constellation::new(
                        10,
                        "Test Constellation".to_string(),
                        vec![9],
                        Region::new(11, "Test Region", vec![10]),
                    ),
                    CostIndexes {
                        manufacturing: 0.456,
                        invention: 0.789,
                    },
                ),
                Some(vec![FacilityUsage::Industry]),
                activites,
            ),
        ];
        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_load_registered_items() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator.load_registered_items().await.unwrap();

        let expected = vec![
            Item::new(18, "Item 18", Some(2.5), TechLevel::One),
            Item::new(19, "Item 19", None, TechLevel::One),
            Item::new(20, "Item 20", Some(123.0), TechLevel::Two),
        ];

        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_load_item() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator.load_item(18).await.unwrap();

        let expected = Item::new(18, "Item 18", Some(2.5), TechLevel::One);

        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_load_system() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator.load_system(9).await.unwrap();

        let expected = SolarSystem::new(
            9,
            "Test Solar System".to_string(),
            0.1234,
            vec![8],
            Constellation::new(
                10,
                "Test Constellation".to_string(),
                vec![9],
                Region::new(11, "Test Region", vec![10]),
            ),
            CostIndexes {
                manufacturing: 0.456,
                invention: 0.789,
            },
        );

        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_load_constellation() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator.load_constellation(10).await.unwrap();

        let expected = Constellation::new(
            10,
            "Test Constellation".to_string(),
            vec![9],
            Region::new(11, "Test Region", vec![10]),
        );

        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_load_region() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator.load_region(11).await.unwrap();

        let expected = Region::new(11, "Test Region", vec![10]);

        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_load_corporation() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator.load_corporation(2).await.unwrap();

        let expected = Corporation {
            id: 2,
            name: "Test Corp".to_string(),
            alliance: Some(Alliance {
                id: 3,
                name: "Test Alliance".to_string(),
            }),
        };

        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_load_manufacturing_blueprints() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator
            .load_blueprints(IndustryType::Manufacturing)
            .await
            .unwrap();

        let mut expected = HashMap::new();
        expected.insert(
            24,
            Blueprint {
                id: 24,
                activities: Activities {
                    manufacturing: Some(BlueprintManufacturing {
                        blueprint_id: 24,
                        materials: Materials::new(vec![]),
                        products: vec![MultipleItems {
                            quantity: 10,
                            item: Item::new(19, "Item 19", None, TechLevel::One),
                        }],
                        material_efficiency: 0,
                        time_efficiency: 0,
                        time: 1054,
                        invention_blueprint: vec![],
                    }),
                    invention: None,
                },
            },
        );
        expected.insert(
            21,
            Blueprint {
                id: 21,
                activities: Activities {
                    manufacturing: Some(BlueprintManufacturing {
                        blueprint_id: 21,
                        materials: Materials::new(vec![]),
                        products: vec![MultipleItems {
                            quantity: 1,
                            item: Item::new(20, "Item 20", Some(123.0), TechLevel::Two),
                        }],
                        material_efficiency: 0,
                        time_efficiency: 0,
                        time: 100,
                        invention_blueprint: vec![BlueprintInvention {
                            blueprint_id: 22,
                            materials: Materials::new(vec![]),
                            products: vec![ProbableMultipleItems {
                                quantity: 1,
                                base_probability: Some(0.3),
                                item: Item::new(
                                    21,
                                    "Item 20 Manufacturing Blueprint",
                                    None,
                                    TechLevel::One,
                                ),
                            }],
                            skills: vec![],
                            time: 100,
                        }],
                    }),
                    invention: None,
                },
            },
        );

        assert_eq!(got.len(), expected.keys().len());
        for bp in got {
            assert_eq!(Some(&bp), expected.get(&bp.id));
        }
    }

    #[tokio::test]
    pub async fn test_load_all_items_with_manufacturing_blueprint() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator
            .load_all_items_with_blueprint(IndustryType::Manufacturing)
            .await
            .unwrap();

        let mut expected = HashMap::new();
        expected.insert(19, Item::new(19, "Item 19", None, TechLevel::One));
        expected.insert(20, Item::new(20, "Item 20", Some(123.0), TechLevel::Two));

        assert_eq!(got.len(), expected.keys().len());
        for item in got {
            assert_eq!(Some(&item), expected.get(&item.id()));
        }
    }

    #[tokio::test]
    pub async fn test_load_item_blueprints() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator
            .load_item_blueprints(19, IndustryType::Manufacturing)
            .await
            .unwrap();

        let expected = vec![Blueprint {
            id: 24,
            activities: Activities {
                manufacturing: Some(BlueprintManufacturing {
                    blueprint_id: 24,
                    materials: Materials::new(vec![]),
                    products: vec![MultipleItems {
                        quantity: 10,
                        item: Item::new(19, "Item 19", None, TechLevel::One),
                    }],
                    material_efficiency: 0,
                    time_efficiency: 0,
                    time: 1054,
                    invention_blueprint: vec![],
                }),
                invention: None,
            },
        }];

        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_search_items() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator
            .search_items(LoadFrom::Name("Item 1".to_string()), false)
            .await
            .unwrap();

        let mut expected = HashMap::new();
        expected.insert(18, Item::new(18, "Item 18", Some(2.5), TechLevel::One));
        expected.insert(19, Item::new(19, "Item 19", None, TechLevel::One));
        expected.insert(
            24,
            Item::new(24, "Item 19 Manufacturing Blueprint", None, TechLevel::One),
        );

        assert_eq!(
            got.len(),
            expected.keys().len(),
            "Got: {:?} - Expected: {:?}",
            got,
            expected
        );
        for item in got {
            assert_eq!(Some(&item), expected.get(&item.id()));
        }
    }

    #[tokio::test]
    pub async fn test_search_structures() {
        let (data_integrator, requester) = create_test_data_integrator();
        let got = data_integrator.search_structure("Test").await.unwrap();

        let mut activites = HashMap::new();
        activites.insert(
            IndustryType::Manufacturing,
            PlayerStructureStats {
                tax_rate: 0.341,
                job_duration_modifier: Some(3.0),
                job_cost_modifier: Some(5.0),
                material_consumption_modifier: Some(2.5),
            },
        );
        activites.insert(
            IndustryType::Invention,
            PlayerStructureStats {
                tax_rate: 0.32,
                job_duration_modifier: Some(3.5),
                job_cost_modifier: Some(3.0),
                material_consumption_modifier: Some(3.5),
            },
        );
        let mut expected = HashMap::new();
        expected.insert(
            15,
            Facility::new_structure(
                requester.clone(),
                15,
                "Test Structure".to_string(),
                SolarSystem::new(
                    9,
                    "Test Solar System".to_string(),
                    0.1234,
                    vec![8],
                    Constellation::new(
                        10,
                        "Test Constellation".to_string(),
                        vec![9],
                        Region::new(11, "Test Region", vec![10]),
                    ),
                    CostIndexes {
                        manufacturing: 0.456,
                        invention: 0.789,
                    },
                ),
                Some(vec![FacilityUsage::Industry]),
                activites,
            ),
        );

        assert_eq!(
            got.len(),
            expected.keys().len(),
            "Got: {:?} - Expected: {:?}",
            got,
            expected
        );
        for item in got {
            assert_eq!(Some(&item), expected.get(&item.id()));
        }
    }

    #[tokio::test]
    pub async fn test_load_prices() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator.load_prices().await.unwrap();

        let mut prices = HashMap::new();
        prices.insert(18, ItemPrice::new(Some(18.1), Some(18.2)));
        prices.insert(19, ItemPrice::new(Some(19.1), Some(19.2)));
        prices.insert(20, ItemPrice::new(Some(20.1), Some(20.2)));
        let expected = Prices { prices };

        assert_eq!(got, expected);
    }

    #[tokio::test]
    pub async fn test_load_all_regions() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator.load_all_regions().await.unwrap();

        let mut expected = HashMap::new();
        expected.insert(11, Region::new(11, "Test Region", vec![10]));

        assert_eq!(
            got.len(),
            expected.keys().len(),
            "Got: {:?} - Expected: {:?}",
            got,
            expected
        );
        for region in got {
            assert_eq!(Some(&region), expected.get(&region.id()));
        }
    }

    #[tokio::test]
    pub async fn test_load_character_industry_jobs() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator
            .load_character_industry_jobs()
            .await
            .unwrap();

        let mut expected = HashMap::new();
        expected.insert(
            19,
            Job::new(
                IndustryType::Manufacturing,
                Some(Item::new(19, "Item 19", None, TechLevel::One)),
                6,
                Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap(),
            ),
        );
        expected.insert(
            21,
            Job::new(
                IndustryType::Invention,
                Some(Item::new(
                    21,
                    "Item 20 Manufacturing Blueprint",
                    None,
                    TechLevel::One,
                )),
                2,
                Utc.with_ymd_and_hms(2014, 10, 2, 10, 11, 12).unwrap(),
            ),
        );

        assert_eq!(
            got.len(),
            expected.keys().len(),
            "Got: {:?} - Expected: {:?}",
            got,
            expected
        );
        for job in got {
            assert_eq!(
                Some(&job),
                expected.get(&job.item_produced.as_ref().unwrap().id())
            );
        }
    }

    #[tokio::test]
    pub async fn test_load_character_orders() {
        let (data_integrator, _) = create_test_data_integrator();
        let got = data_integrator.load_character_orders().await.unwrap();

        let mut expected = HashMap::new();
        expected.insert(
            19,
            CharacterOrder {
                item: Item::new(19, "Item 19", None, TechLevel::One),
                order_type: OrderType::Sell,
                price: 123456.78,
                volume_remain: 456,
                volume_total: 789,
            },
        );
        expected.insert(
            20,
            CharacterOrder {
                item: Item::new(20, "Item 20", Some(123.0), TechLevel::Two),
                order_type: OrderType::Buy,
                price: 456789.12,
                volume_remain: 789,
                volume_total: 1230,
            },
        );

        assert_eq!(
            got.len(),
            expected.keys().len(),
            "Got: {:?} - Expected: {:?}",
            got,
            expected
        );
        for order in got {
            assert_eq!(Some(&order), expected.get(&order.item.id()));
        }
    }
}
