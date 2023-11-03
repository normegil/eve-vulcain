use crate::model::common::{Identified, Named};
use rfesi::groups::CostIndex;

#[derive(Debug, PartialEq, Clone)]
pub struct SolarSystem {
    id: i32,
    name: String,
    pub security_status: f64,
    pub station_ids: Vec<i32>,
    pub constellation: Constellation,
    pub indexes: CostIndexes,
}

impl SolarSystem {
    pub fn new(
        id: i32,
        name: String,
        security_status: f64,
        station_ids: Vec<i32>,
        constellation: Constellation,
        indexes: CostIndexes,
    ) -> Self {
        Self {
            id,
            name,
            security_status,
            station_ids,
            constellation,
            indexes,
        }
    }
}

impl Identified<i32> for SolarSystem {
    fn id(&self) -> i32 {
        self.id
    }
}

impl Named for SolarSystem {
    fn name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Constellation {
    id: i32,
    name: String,
    pub system_ids: Vec<i32>,
    pub region: Region,
}

impl Constellation {
    pub fn new(id: i32, name: String, system_ids: Vec<i32>, region: Region) -> Self {
        Self {
            id,
            name,
            system_ids,
            region,
        }
    }
}

impl Identified<i32> for Constellation {
    fn id(&self) -> i32 {
        self.id
    }
}

impl Named for Constellation {
    fn name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Region {
    id: i32,
    name: String,
    pub constellation_ids: Vec<i32>,
}

impl Region {
    pub fn new(id: i32, name: &str, constellation_ids: Vec<i32>) -> Self {
        Self {
            id,
            name: name.to_string(),
            constellation_ids,
        }
    }
}

impl Identified<i32> for Region {
    fn id(&self) -> i32 {
        self.id
    }
}

impl Named for Region {
    fn name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct CostIndexes {
    pub manufacturing: f64,
    pub invention: f64,
}

impl From<&Vec<CostIndex>> for CostIndexes {
    fn from(indexes: &Vec<CostIndex>) -> Self {
        let mut manufacturing_index = 0.0;
        let mut invention_index = 0.0;

        for index in indexes {
            if "manufacturing" == index.activity {
                manufacturing_index = index.cost_index;
            } else if "invention" == index.activity {
                invention_index = index.cost_index;
            }
        }

        Self {
            manufacturing: manufacturing_index,
            invention: invention_index,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_from_empty_indexes() {
        let indexes: Vec<CostIndex> = vec![];
        let cost_indexes = CostIndexes::from(&indexes);

        assert_eq!(
            cost_indexes,
            CostIndexes {
                manufacturing: 0.0,
                invention: 0.0
            }
        );
    }

    #[test]
    fn test_from_single_manufacturing_index() {
        let indexes = vec![CostIndex {
            activity: String::from("manufacturing"),
            cost_index: 10.0,
        }];
        let cost_indexes = CostIndexes::from(&indexes);

        assert_eq!(
            cost_indexes,
            CostIndexes {
                manufacturing: 10.0,
                invention: 0.0
            }
        );
    }

    #[test]
    fn test_from_single_invention_index() {
        let indexes = vec![CostIndex {
            activity: String::from("invention"),
            cost_index: 15.0,
        }];
        let cost_indexes = CostIndexes::from(&indexes);

        assert_eq!(
            cost_indexes,
            CostIndexes {
                manufacturing: 0.0,
                invention: 15.0
            }
        );
    }

    #[test]
    fn test_from_multiple_indexes() {
        let indexes = vec![
            CostIndex {
                activity: String::from("manufacturing"),
                cost_index: 10.0,
            },
            CostIndex {
                activity: String::from("invention"),
                cost_index: 15.0,
            },
        ];
        let cost_indexes = CostIndexes::from(&indexes);

        assert_eq!(
            cost_indexes,
            CostIndexes {
                manufacturing: 10.0,
                invention: 15.0
            }
        );
    }

    #[test]
    fn test_from_multiple_indexes_with_duplicate_activity() {
        let indexes = vec![
            CostIndex {
                activity: String::from("manufacturing"),
                cost_index: 10.0,
            },
            CostIndex {
                activity: String::from("invention"),
                cost_index: 15.0,
            },
            CostIndex {
                activity: String::from("manufacturing"),
                cost_index: 20.0,
            },
        ];
        let cost_indexes = CostIndexes::from(&indexes);

        assert_eq!(
            cost_indexes,
            CostIndexes {
                manufacturing: 20.0,
                invention: 15.0
            }
        );
    }
}
