use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::num::TryFromIntError;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use strum_macros::{EnumIter, EnumString};
use thiserror::Error;

use crate::api::evecache::MarketTraits;
use crate::model::common::{Identified, Named};
use crate::model::locations::SolarSystem;

use self::invention::InventionFacility;
use self::manufacture::Manufacture;
use self::markets::Market;
use self::npcstation::NPCStation;
use self::playerstructure::{PlayerStructure, PlayerStructureStats};

use super::industry::IndustryType;

pub mod invention;
pub mod manufacture;
pub mod markets;
pub mod npcstation;
pub mod playerstructure;

#[derive(Clone)]
pub struct Facility {
    eve: Arc<dyn MarketTraits>,

    name: String,
    pub location: SolarSystem,
    pub registered_usages: Option<Vec<FacilityUsage>>,
    type_specific_data: FacilityType,
}

impl Debug for Facility {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Facility")
            .field("name", &self.name)
            .field("location", &self.location)
            .field("registered_usages", &self.registered_usages)
            .field("type_specific_data", &self.type_specific_data)
            .finish()
    }
}

impl PartialEq for Facility {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.location == other.location
            && self.registered_usages == other.registered_usages
            && self.type_specific_data == other.type_specific_data
    }
}

impl Facility {
    pub fn new_station(
        eve: Arc<dyn MarketTraits>,
        id: i32,
        name: String,
        location: SolarSystem,
        registered_usages: Option<Vec<FacilityUsage>>,
    ) -> Self {
        Self {
            eve,
            name,
            location,
            registered_usages,
            type_specific_data: FacilityType::Station(NPCStation::new(id)),
        }
    }

    pub fn new_structure(
        eve: Arc<dyn MarketTraits>,
        id: i64,
        name: String,
        location: SolarSystem,
        registered_usages: Option<Vec<FacilityUsage>>,
        industrial_activities: HashMap<IndustryType, PlayerStructureStats>,
    ) -> Self {
        Self {
            eve,
            name,
            location,
            registered_usages,
            type_specific_data: FacilityType::Structure(PlayerStructure::new(
                id,
                industrial_activities,
            )),
        }
    }

    pub fn facility_type(&self) -> FacilityType {
        self.type_specific_data.clone()
    }

    pub fn is_market(&self) -> bool {
        self.registered_usages.is_some()
            && self
                .registered_usages
                .as_ref()
                .unwrap()
                .contains(&FacilityUsage::Market)
    }

    pub fn support_industry_type(&self, industry_type: &IndustryType) -> bool {
        match &self.type_specific_data {
            FacilityType::Station(_) => {
                industry_type != &IndustryType::Reaction || self.location.security_status < 5.0
            }
            FacilityType::Structure(s) => s.activities.contains_key(industry_type),
        }
    }

    pub fn market(&self) -> Option<Market> {
        if self.is_market() {
            Some(Market::new(self))
        } else {
            None
        }
    }

    pub fn manufacture(&self) -> Option<Manufacture> {
        if self.support_industry_type(&IndustryType::Manufacturing) {
            Some(Manufacture::new(self))
        } else {
            None
        }
    }

    pub fn invention(&self) -> Option<InventionFacility> {
        if self.support_industry_type(&IndustryType::Invention) {
            Some(InventionFacility::new(self))
        } else {
            None
        }
    }
}

#[derive(Debug, Error)]
#[error("Identifier conversion failed ({identified_entity}): {source}")]
pub struct IdentifierTypeConversionFailed {
    identified_entity: String,
    source: TryFromIntError,
}

impl Identified<i64> for Facility {
    fn id(&self) -> i64 {
        match &self.type_specific_data {
            FacilityType::Station(s) => s.id() as i64,
            FacilityType::Structure(s) => s.id(),
        }
    }
}

impl Named for Facility {
    fn name(&self) -> String {
        self.name.clone()
    }
}

#[derive(EnumString, EnumIter, Eq, PartialEq, Hash, Serialize, Deserialize, Clone, Debug)]
pub enum FacilityUsage {
    Market,
    Industry,
}

#[derive(Debug, PartialEq, EnumIter, EnumString, Clone)]
pub enum FacilityType {
    Station(NPCStation),
    Structure(PlayerStructure),
}

impl Display for FacilityType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FacilityType::Station(_) => {
                write!(f, "Station")
            }
            FacilityType::Structure(_) => {
                write!(f, "Structure")
            }
        }
    }
}

// pub struct Order {
//     pub quantity: i32,
//     pub price: f64,
// }
