use std::collections::HashMap;

use serde::Deserialize;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::cache::FSCache;

#[derive(Debug, Error)]
pub enum SDEErrors {
    #[error("load '{path}': {source}")]
    CacheRequestFailed {
        path: String,
        source: crate::cache::FSCacheReadError,
    },
    #[error("deserialize yaml from '{path}': {source}")]
    YAMLDeserializationError {
        path: String,
        source: serde_yaml::Error,
    },
}

pub struct Sde {
    cache: FSCache,

    blueprints_cache: RwLock<Option<HashMap<i32, SDEBlueprint>>>,
}

impl Sde {
    pub fn new(cache: FSCache) -> Self {
        Self {
            cache,
            blueprints_cache: RwLock::new(None),
        }
    }

    pub async fn load_blueprints(&self) -> Result<HashMap<i32, SDEBlueprint>, SDEErrors> {
        if self.blueprints_cache.read().await.is_none() {
            let mut cache = self.blueprints_cache.write().await;
            if cache.is_none() {
                let yaml_path = "sde/fsd/blueprints.yaml";
                let content = self.cache.load_full(yaml_path).await.map_err(|source| {
                    SDEErrors::CacheRequestFailed {
                        path: yaml_path.to_string(),
                        source,
                    }
                })?;
                let blueprints: HashMap<i32, SDEBlueprint> = serde_yaml::from_str(&content)
                    .map_err(|source| SDEErrors::YAMLDeserializationError {
                        path: yaml_path.to_string(),
                        source,
                    })?;
                *cache = Some(blueprints);
            }
        }
        Ok(self
            .blueprints_cache
            .read()
            .await
            .clone()
            .expect("Cache should be already filled here"))
    }

    pub async fn load_blueprint(
        &self,
        type_id: i32,
        activity: &BlueprintActivityType,
    ) -> Result<Option<(i32, SDEBlueprint)>, SDEErrors> {
        let blueprints = self.load_blueprints().await?;
        for (blueprint_id, blueprint) in blueprints {
            if blueprint.can_produce(type_id, activity) {
                return Ok(Some((blueprint_id, blueprint.clone())));
            }
        }
        Ok(None)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct SDEBlueprint {
    pub activities: Activities,
    #[serde(rename = "blueprintTypeID")]
    pub blueprint_type_id: i32,
    #[serde(rename = "maxProductionLimit")]
    pub max_production_limit: i32,
}

impl SDEBlueprint {
    pub fn can_produce(&self, item_id: i32, activity_type: &BlueprintActivityType) -> bool {
        match activity_type {
            BlueprintActivityType::Manufacturing => {
                if let Some(manufacturing) = &self.activities.manufacturing {
                    if let Some(products) = &manufacturing.products {
                        for product in products {
                            if product.type_id == item_id {
                                return true;
                            }
                        }
                    }
                }
            }
            BlueprintActivityType::Invention => {
                if let Some(invention) = &self.activities.invention {
                    if let Some(products) = &invention.products {
                        for product in products {
                            if product.type_id == item_id {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Activities {
    pub copying: Option<Copying>,
    pub invention: Option<Invention>,
    pub manufacturing: Option<Manufacturing>,
    pub research_material: Option<ResearchMaterial>,
    pub research_time: Option<ResearchTime>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Copying {
    pub time: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Invention {
    pub materials: Option<Vec<Item>>,
    pub products: Option<Vec<ProbableMultipleItems>>,
    pub skills: Option<Vec<Skills>>,
    pub time: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Manufacturing {
    pub materials: Option<Vec<Item>>,
    pub products: Option<Vec<Item>>,
    pub time: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ResearchMaterial {
    pub time: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ResearchTime {
    pub time: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Item {
    pub quantity: i32,
    #[serde(rename = "typeID")]
    pub type_id: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ProbableMultipleItems {
    pub probability: Option<f64>,
    pub quantity: i32,
    #[serde(rename = "typeID")]
    pub type_id: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Skills {
    pub level: i32,
    #[serde(rename = "typeID")]
    pub type_id: i32,
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub enum BlueprintActivityType {
    Manufacturing,
    Invention,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdeblueprint_can_produce() {
        let json_data = r#"
            {
                "activities": {
                    "manufacturing": {
                        "products": [
                            { "typeID": 1, "quantity": 1 },
                            { "typeID": 2, "quantity": 1 },
                            { "typeID": 3, "quantity": 1 }
                        ],
                        "time": 100
                    },
                    "invention": {
                        "products": [
                            { "typeID": 4, "quantity": 1, "probability": 0.3 },
                            { "typeID": 5, "quantity": 1, "probability": 0.3 }
                        ],
                        "time": 100
                    }
                },
                "blueprintTypeID": 123,
                "maxProductionLimit": 10
            }
        "#;

        let blueprint: SDEBlueprint = serde_json::from_str(json_data).unwrap();

        assert!(blueprint.can_produce(2, &BlueprintActivityType::Manufacturing));
        assert!(!blueprint.can_produce(6, &BlueprintActivityType::Manufacturing));

        assert!(blueprint.can_produce(4, &BlueprintActivityType::Invention));
        assert!(!blueprint.can_produce(1, &BlueprintActivityType::Invention));
    }
}
