use std::fmt::{Display, Formatter};

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use strum_macros::{EnumIter, EnumString};
use thiserror::Error;

use super::items::Item;

#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub industry_type: IndustryType,
    pub item_produced: Option<Item>,
    pub end_date: chrono::DateTime<Utc>,
    pub runs: i32,
}

impl Job {
    pub fn new(
        industry_type: IndustryType,
        item_produced: Option<Item>,
        runs: i32,
        end_date: chrono::DateTime<Utc>,
    ) -> Self {
        Self {
            industry_type,
            item_produced,
            runs,
            end_date,
        }
    }

    pub fn duration_left(&self) -> Duration {
        let now = Utc::now();
        if Utc::now() > self.end_date {
            return Duration::zero();
        }
        self.end_date - now
    }
}

#[derive(Debug, Error)]
#[error("Industry type not found for id '{id}'")]
pub struct IndustryTypeNotFound {
    id: i32,
}

#[derive(Copy, EnumString, EnumIter, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum IndustryType {
    Manufacturing,
    ResearchTimeEfficiency,
    ResearchMaterialEfficiency,
    Copying,
    Invention,
    Reaction,
}

impl TryFrom<i32> for IndustryType {
    type Error = IndustryTypeNotFound;

    fn try_from(id: i32) -> Result<Self, Self::Error> {
        match id {
            1 => Ok(IndustryType::Manufacturing),
            3 => Ok(IndustryType::ResearchTimeEfficiency),
            4 => Ok(IndustryType::ResearchMaterialEfficiency),
            5 => Ok(IndustryType::Copying),
            8 => Ok(IndustryType::Invention),
            11 => Ok(IndustryType::Reaction),
            _ => Err(IndustryTypeNotFound { id }),
        }
    }
}

impl IndustryType {
    #[cfg(test)]
    pub fn to_activity_id(self) -> i32 {
        match self {
            IndustryType::Manufacturing => 1,
            IndustryType::ResearchTimeEfficiency => 3,
            IndustryType::ResearchMaterialEfficiency => 4,
            IndustryType::Copying => 5,
            IndustryType::Invention => 8,
            IndustryType::Reaction => 11,
        }
    }
}

impl Display for IndustryType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let res = match self {
            IndustryType::Manufacturing => "Manufacturing",
            IndustryType::Copying => "Copying",
            IndustryType::Invention => "Invention",
            IndustryType::Reaction => "Reaction",
            IndustryType::ResearchTimeEfficiency => "Material Efficiency Research",
            IndustryType::ResearchMaterialEfficiency => "Time Efficiency Research",
        };
        write!(f, "{}", res)
    }
}
