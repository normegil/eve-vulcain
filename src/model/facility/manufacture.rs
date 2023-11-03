use futures_util::future::JoinAll;
use thiserror::Error;
use tokio::join;

use crate::{
    logging,
    model::{
        blueprint::BlueprintManufacturing,
        character::{ManufacturingSkills, Skills},
        common::Named,
        items::TechLevel,
        prices::Prices,
    },
    vector::{UnicityError, UniqueElement},
};

use super::{invention::InventionFacility, Facility, FacilityType};

#[derive(Debug, Error)]
pub enum ManufactureError {
    #[error("Products in blueprint '{blueprint_id}' is not unique: {source}")]
    BlueprintProductUnicityError {
        blueprint_id: i32,
        source: UnicityError,
    },
}

pub trait ManufacturingFacility {
    fn manufacturing_tax(&self) -> f64;
    fn job_cost_modifier(&self) -> Option<f64>;
    fn job_duration_modifier(&self) -> Option<f64>;
    fn material_consumption_modifier(&self) -> Option<f64>;
}

pub struct Manufacture<'a>(&'a Facility);

impl<'a> Manufacture<'a> {
    pub fn new(facility: &'a Facility) -> Self {
        Self(facility)
    }

    pub async fn manufacture_cost_per_run(
        &self,
        blueprint: &BlueprintManufacturing,
        skills: &Skills,
        inventions_facilities: &Vec<InventionFacility<'_>>,
        prices: &Prices,
    ) -> Result<f64, ManufactureError> {
        let mut blueprint = blueprint.clone();

        let mut blueprint_run_price = 0.0;
        let item = &blueprint
            .products
            .unique_ref("Only unique products supported")
            .map_err(|source| ManufactureError::BlueprintProductUnicityError {
                blueprint_id: blueprint.blueprint_id,
                source,
            })?
            .item;
        if TechLevel::Two == item.tech_level {
            let mut futures = vec![];
            for invention in inventions_facilities {
                futures.push(invention.invent_item(&blueprint, skills, prices));
            }
            let all_futures = futures.into_iter().collect::<JoinAll<_>>();
            let invention_result = join!(all_futures).0;
            let cheapest_invention_result = invention_result.iter().reduce(|acc, res| {
                if acc.value.cost_normalized > res.value.cost_normalized {
                    res
                } else {
                    acc
                }
            });
            if let Some(res) = cheapest_invention_result {
                let total_per_run = res.value.cost_normalized / (res.value.blueprint.runs as f64);

                blueprint.material_efficiency = res.value.blueprint.material_efficiency;
                blueprint.time_efficiency = res.value.blueprint.time_efficiency;

                blueprint_run_price = total_per_run;
            }
        }

        let material_costs = blueprint.materials.input_materials_cost(
            self.material_consumption_modifier(),
            Some(blueprint.material_efficiency),
            prices,
        );
        let installation_cost = self.job_installation_cost(blueprint.estimated_item_value(prices));
        Ok(material_costs.value + installation_cost + blueprint_run_price)
    }

    pub fn job_installation_cost(&self, estimated_item_value: f64) -> f64 {
        let mut gross_cost = estimated_item_value * self.0.location.indexes.manufacturing;
        logging::trace!("Gross Cost: {}", gross_cost);
        if let Some(modifier) = self.job_cost_modifier() {
            gross_cost -= gross_cost * modifier;
            logging::trace!("Gross Cost (Modifier): {}", gross_cost);
        }

        let mut tax = estimated_item_value * self.manufacturing_tax();
        logging::trace!("Tax: {}", tax);

        // Fixed amount
        // https://wiki.eveuniversity.org/Manufacturing
        // https://www.eveonline.com/news/view/patch-notes-version-21-05-2
        static SCC_SURCHARGE: f64 = 0.0015;

        tax += estimated_item_value * SCC_SURCHARGE;
        logging::trace!("Tax (SCC): {}", tax);

        gross_cost + tax
    }

    pub fn time_per_run(
        &self,
        blueprint_manufacturing: &BlueprintManufacturing,
        skills: &ManufacturingSkills,
    ) -> i32 {
        let time_efficiency_normalized =
            1.0 - (blueprint_manufacturing.time_efficiency as f64) / 100.0;
        logging::trace!("{:?}", time_efficiency_normalized);
        let mut run_time = (blueprint_manufacturing.time as f64) * time_efficiency_normalized;
        if let Some(modifier) = self.job_duration_modifier() {
            let job_duration_normalized = 1.0 - modifier;
            run_time *= job_duration_normalized;
        }

        let mut industry_level = 0;
        if let Some(industry) = &skills.industry {
            industry_level = industry.trained_level
        }

        let mut advanced_industry_level = 0;
        if let Some(advanced_industry) = &skills.advanced_industry_level {
            advanced_industry_level = advanced_industry.trained_level
        }

        let skill_industry_modifier = 0.04 * (industry_level as f64);
        let skill_advanced_industry_modifier = 0.03 * (advanced_industry_level as f64);
        run_time *= (1.0 - skill_industry_modifier) * (1.0 - skill_advanced_industry_modifier);
        run_time as i32
    }

    pub fn facility(&self) -> &Facility {
        self.0
    }
}

impl<'a> Named for Manufacture<'a> {
    fn name(&self) -> String {
        self.0.name()
    }
}

impl<'a> ManufacturingFacility for Manufacture<'a> {
    fn manufacturing_tax(&self) -> f64 {
        match &self.0.type_specific_data {
            FacilityType::Station(s) => s.manufacturing_tax(),
            FacilityType::Structure(s) => s.manufacturing_tax(),
        }
    }

    fn job_cost_modifier(&self) -> Option<f64> {
        match &self.0.type_specific_data {
            FacilityType::Station(s) => ManufacturingFacility::job_cost_modifier(s),
            FacilityType::Structure(s) => ManufacturingFacility::job_cost_modifier(s),
        }
    }

    fn job_duration_modifier(&self) -> Option<f64> {
        match &self.0.type_specific_data {
            FacilityType::Station(s) => ManufacturingFacility::job_duration_modifier(s),
            FacilityType::Structure(s) => ManufacturingFacility::job_duration_modifier(s),
        }
    }

    fn material_consumption_modifier(&self) -> Option<f64> {
        match &self.0.type_specific_data {
            FacilityType::Station(s) => s.material_consumption_modifier(),
            FacilityType::Structure(s) => s.material_consumption_modifier(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use crate::{
        api::evecache::mocks::MockRequester,
        model::{
            blueprint::{BlueprintInvention, Materials, MultipleItems, ProbableMultipleItems},
            facility::{playerstructure::PlayerStructureStats, FacilityUsage},
            industry::IndustryType,
            items::Item,
            locations::{Constellation, CostIndexes, Region, SolarSystem},
            prices::ItemPrice,
            skills::{Skill, TrainedSkill},
        },
        round::Round,
    };

    use super::*;

    fn create_test_facility() -> Facility {
        let mut activites = HashMap::new();
        activites.insert(
            IndustryType::Manufacturing,
            PlayerStructureStats {
                tax_rate: 0.1,
                job_duration_modifier: Some(5.0),
                job_cost_modifier: Some(1.5),
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
        let facility = Facility::new_structure(
            Arc::new(MockRequester::builder().build()),
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
        facility
    }

    fn create_blueprint() -> BlueprintManufacturing {
        BlueprintManufacturing {
            blueprint_id: 10,
            materials: Materials::new(vec![
                MultipleItems {
                    quantity: 100,
                    item: Item::new(100, "Item 100", None, TechLevel::One),
                },
                MultipleItems {
                    quantity: 50,
                    item: Item::new(101, "Item 101", None, TechLevel::One),
                },
            ]),
            products: vec![MultipleItems {
                quantity: 1,
                item: Item::new(200, "Item 101", None, TechLevel::Two),
            }],
            material_efficiency: 5,
            time_efficiency: 8,
            time: 154,
            invention_blueprint: vec![BlueprintInvention {
                blueprint_id: 1,
                materials: Materials::new(vec![
                    MultipleItems {
                        quantity: 10,
                        item: Item::new(50, "Item 50", None, TechLevel::One),
                    },
                    MultipleItems {
                        quantity: 2,
                        item: Item::new(51, "Item 51", None, TechLevel::One),
                    },
                ]),
                products: vec![ProbableMultipleItems {
                    quantity: 2,
                    base_probability: Some(0.3),
                    item: Item::new(10, "test", None, TechLevel::One),
                }],
                skills: vec![
                    Skill::new(10, "Skill1"),
                    Skill::new(11, "Skill2"),
                    Skill::new(12, "Skill3 Encryption Methods"),
                ],
                time: 100,
            }],
        }
    }

    #[test]
    fn test_job_installation_cost() {
        let facility = create_test_facility();

        let estimated_item_value = 123.4;

        let result = facility
            .manufacture()
            .unwrap()
            .job_installation_cost(estimated_item_value);

        assert_eq!(result.specific_round(2), -15.61);
    }

    #[test]
    fn test_time_per_run() {
        let facility = create_test_facility();
        let blueprint = create_blueprint();

        let result = facility.manufacture().unwrap().time_per_run(
            &blueprint,
            &ManufacturingSkills {
                industry: Some(TrainedSkill::new(10, "Industry", 5)),
                advanced_industry_level: Some(TrainedSkill::new(11, "Advanced Industry", 3)),
            },
        );

        assert_eq!(result, -412);
    }

    #[tokio::test]
    async fn test_manufacture_cost_per_run() {
        let facility = create_test_facility();
        let blueprint = create_blueprint();

        let skills = Skills {
            skills: vec![
                TrainedSkill::new(10, "Skill1", 3),
                TrainedSkill::new(11, "Skill2", 1),
                TrainedSkill::new(12, "Skill3 Encryption Methods", 5),
                TrainedSkill::new(13, "Industry", 4),
                TrainedSkill::new(14, "Advanced Industry", 3),
            ],
        };

        let mut prices = HashMap::new();
        prices.insert(50, ItemPrice::new(None, Some(2.5)));
        prices.insert(51, ItemPrice::new(None, Some(3.5)));
        prices.insert(100, ItemPrice::new(None, Some(13.5)));
        prices.insert(101, ItemPrice::new(None, Some(23.5)));
        let prices = Prices { prices };

        let result = facility
            .manufacture()
            .unwrap()
            .manufacture_cost_per_run(
                &blueprint,
                &skills,
                &vec![facility.invention().unwrap()],
                &prices,
            )
            .await
            .unwrap();

        assert_eq!(result.specific_round(2), -3795.62);
    }
}
