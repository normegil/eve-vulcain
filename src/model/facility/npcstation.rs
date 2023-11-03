use crate::model::common::Identified;

use super::{invention::InventionFacilityProperties, manufacture::ManufacturingFacility};

#[derive(Debug, PartialEq, Clone, Default)]
pub struct NPCStation {
    id: i32,
}

impl NPCStation {
    pub fn new(id: i32) -> Self {
        Self { id }
    }
}

impl Identified<i32> for NPCStation {
    fn id(&self) -> i32 {
        self.id
    }
}

impl ManufacturingFacility for NPCStation {
    fn manufacturing_tax(&self) -> f64 {
        0.0025
    }

    fn job_cost_modifier(&self) -> Option<f64> {
        None
    }

    fn job_duration_modifier(&self) -> Option<f64> {
        None
    }

    fn material_consumption_modifier(&self) -> Option<f64> {
        None
    }
}

impl InventionFacilityProperties for NPCStation {
    fn invention_tax(&self) -> f64 {
        0.0025
    }

    fn job_cost_modifier(&self) -> Option<f64> {
        None
    }

    fn job_duration_modifier(&self) -> Option<f64> {
        None
    }
}
