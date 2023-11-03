use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::model::{common::Identified, industry::IndustryType};

use super::{invention::InventionFacilityProperties, manufacture::ManufacturingFacility};

#[derive(Debug, PartialEq, Clone, Default)]
pub struct PlayerStructure {
    id: i64,
    pub activities: HashMap<IndustryType, PlayerStructureStats>,
}

impl PlayerStructure {
    pub fn new(id: i64, activities: HashMap<IndustryType, PlayerStructureStats>) -> Self {
        Self { id, activities }
    }
}

impl Identified<i64> for PlayerStructure {
    fn id(&self) -> i64 {
        self.id
    }
}

impl ManufacturingFacility for PlayerStructure {
    fn manufacturing_tax(&self) -> f64 {
        match self.activities.get(&IndustryType::Manufacturing) {
            None => 0.0,
            Some(stat) => stat.tax_rate,
        }
    }

    fn job_cost_modifier(&self) -> Option<f64> {
        match self.activities.get(&IndustryType::Manufacturing) {
            None => None,
            Some(stat) => stat.job_cost_modifier,
        }
    }

    fn job_duration_modifier(&self) -> Option<f64> {
        match self.activities.get(&IndustryType::Manufacturing) {
            None => None,
            Some(stat) => stat.job_duration_modifier,
        }
    }

    fn material_consumption_modifier(&self) -> Option<f64> {
        match self.activities.get(&IndustryType::Manufacturing) {
            None => None,
            Some(stat) => stat.material_consumption_modifier,
        }
    }
}

impl InventionFacilityProperties for PlayerStructure {
    fn invention_tax(&self) -> f64 {
        match self.activities.get(&IndustryType::Invention) {
            None => 0.0,
            Some(stat) => stat.tax_rate,
        }
    }

    fn job_cost_modifier(&self) -> Option<f64> {
        match self.activities.get(&IndustryType::Invention) {
            None => None,
            Some(stat) => stat.job_cost_modifier,
        }
    }

    fn job_duration_modifier(&self) -> Option<f64> {
        match self.activities.get(&IndustryType::Invention) {
            None => None,
            Some(stat) => stat.job_duration_modifier,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerStructureStats {
    pub(crate) tax_rate: f64,
    pub(crate) job_duration_modifier: Option<f64>,
    pub(crate) job_cost_modifier: Option<f64>,
    pub(crate) material_consumption_modifier: Option<f64>,
}
