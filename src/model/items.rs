use crate::model::common::{Identified, Named};
use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Clone)]
pub struct Item {
    id: i32,
    name: String,
    pub volume: Option<f64>,
    pub tech_level: TechLevel,
}

impl Item {
    pub fn new(id: i32, name: &str, volume: Option<f64>, tech_level: TechLevel) -> Self {
        Self {
            id,
            name: name.to_string(),
            volume,
            tech_level,
        }
    }
}

impl Identified<i32> for Item {
    fn id(&self) -> i32 {
        self.id
    }
}

impl Named for Item {
    fn name(&self) -> String {
        self.name.clone()
    }
}

impl Display for Item {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TechLevel {
    One,
    Two,
}
