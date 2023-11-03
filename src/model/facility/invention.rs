use crate::{
    logging,
    model::{
        blueprint::{BlueprintInvention, BlueprintManufacturing, InputMaterialsCostsDetails},
        character::ManufacturingSkills,
        character::Skills,
        common::DetailedCalculation,
        common::{Identified, Named},
        prices::Prices,
        skills::TrainedSkill,
    },
};

use super::{Facility, FacilityType};

pub trait InventionFacilityProperties {
    fn invention_tax(&self) -> f64;
    fn job_cost_modifier(&self) -> Option<f64>;
    fn job_duration_modifier(&self) -> Option<f64>;
}

pub struct InventionFacility<'a>(&'a Facility);

impl<'a> InventionFacility<'a> {
    pub fn new(facility: &'a Facility) -> Self {
        Self(facility)
    }

    pub fn facility(&self) -> &Facility {
        self.0
    }

    pub fn job_installation_cost(
        &self,
        estimated_item_value: f64,
    ) -> DetailedCalculation<f64, JobInventionCostDetails> {
        let base_job_cost = 0.02 * estimated_item_value;
        logging::trace!("Base job cost: {}", base_job_cost);
        let mut gross_cost = base_job_cost * self.0.location.indexes.invention;
        logging::trace!("Gross Cost: {}", gross_cost);
        if let Some(modifier) = self.job_cost_modifier() {
            gross_cost -= gross_cost * modifier;
            logging::trace!("Gross Cost (Modifier): {}", gross_cost);
        }

        let mut tax = base_job_cost * self.invention_tax();
        logging::trace!("Tax: {}", tax);

        // Fixed amount
        // https://wiki.eveuniversity.org/Manufacturing
        // https://www.eveonline.com/news/view/patch-notes-version-21-05-2
        static SCC_SURCHARGE: f64 = 0.015;

        tax += base_job_cost * SCC_SURCHARGE;
        logging::trace!("Tax (SCC): {}", tax);

        DetailedCalculation {
            value: gross_cost + tax,
            details: JobInventionCostDetails {
                estimated_item_value,
                base_job_cost,
                job_cost_modifier: self.job_cost_modifier(),
                facility_invention_tax: self.invention_tax(),
                scc_surcharge: SCC_SURCHARGE,
            },
        }
    }

    pub fn time_per_run(
        &self,
        blueprint: &BlueprintInvention,
        skills: &ManufacturingSkills,
    ) -> DetailedCalculation<i32, TimeDetails> {
        let mut run_time = blueprint.time as f64;
        if let Some(modifier) = self.job_duration_modifier() {
            let job_duration_normalized = 1.0 - modifier;
            run_time *= job_duration_normalized;
        }

        let mut advanced_industry_level = 0;
        if let Some(advanced_industry) = &skills.advanced_industry_level {
            advanced_industry_level = advanced_industry.trained_level
        }

        let skill_advanced_industry_modifier = 0.03 * (advanced_industry_level as f64);
        run_time *= 1.0 - skill_advanced_industry_modifier;

        DetailedCalculation {
            value: run_time as i32,
            details: TimeDetails {
                // base_time: blueprint.time,
                // job_duration_modifier: self.job_duration_modifier(),
                // advanced_industry_skill_level: advanced_industry_level,
            },
        }
    }

    pub async fn success_probability(
        &self,
        invention_blueprint: &BlueprintInvention,
        product_id: i32,
        skills: &Skills,
    ) -> DetailedCalculation<f64, SuccessProbabilityDetails> {
        let base_success_chance= invention_blueprint
            .get_product(product_id)
            .unwrap_or_else(|| panic!("Loaded invention blueprint as been loaded based on the product id (Blueprint ID:{};Invention Product ID{})",invention_blueprint.blueprint_id, product_id))
            .base_probability
            .unwrap_or(1.0);

        let mut invention_skills = vec![];
        for skill in &invention_blueprint.skills {
            let found_skill = skills.get_skill(skill.id());
            invention_skills.push(match found_skill {
                Some(s) => s.clone(),
                None => TrainedSkill::new(skill.id(), &skill.name(), 0),
            });
        }

        let mut all_science_skills = 0.0;
        let mut encryption_skills_level = 0.0;
        for skill in &invention_skills {
            if skill.name().ends_with("Encryption Methods") {
                encryption_skills_level += skill.trained_level as f64;
            } else {
                all_science_skills += skill.trained_level as f64;
            }
        }
        let final_success_chance = base_success_chance
            * (1.0 + (all_science_skills / 30.0) + (encryption_skills_level / 40.0));

        DetailedCalculation {
            value: final_success_chance,
            details: SuccessProbabilityDetails {
                base_success_chance,
                skills: invention_skills,
                final_success_chance,
            },
        }
    }

    pub async fn invent_item(
        &self,
        manufacturing_blueprint: &BlueprintManufacturing,
        skills: &Skills,
        prices: &Prices,
    ) -> DetailedCalculation<InventionResult, InventionDetails> {
        let invention_blueprint: &BlueprintInvention =
            &manufacturing_blueprint.invention_blueprint[0];
        let input_materials_cost = invention_blueprint
            .materials
            .input_materials_cost(None, None, prices);
        let estimated_item_value = manufacturing_blueprint.estimated_item_value(prices);
        let job_installation_cost = self.job_installation_cost(estimated_item_value);
        let cost_per_run = job_installation_cost.value + input_materials_cost.value;
        let time_per_run =
            self.time_per_run(invention_blueprint, &skills.get_manufacturing_skill());
        let success_probability = self
            .success_probability(
                invention_blueprint,
                manufacturing_blueprint.blueprint_id,
                skills,
            )
            .await;

        let normalization_factor = 1.0 / success_probability.value;

        let nb_runs = invention_blueprint
            .get_product(manufacturing_blueprint.blueprint_id)
            .unwrap_or_else(|| panic!("Product should exist because the invention blueprint (id:{}) has been loaded based on the product", invention_blueprint.blueprint_id))
            .quantity;

        DetailedCalculation {
            value: InventionResult {
                cost_normalized: cost_per_run * normalization_factor,
                time_normalized: (time_per_run.value as f64) * normalization_factor,
                blueprint: InventionResultBlueprint {
                    id: manufacturing_blueprint.blueprint_id,
                    runs: nb_runs,
                    material_efficiency: 2,
                    time_efficiency: 4,
                },
            },
            details: InventionDetails {
                cost: CostsDetails {
                    inputs: input_materials_cost.details,
                    job_cost: job_installation_cost.details,
                    total: cost_per_run,
                },
                time: time_per_run.details,
                success_probability: success_probability.details,
            },
        }
    }
}

impl<'a> InventionFacilityProperties for InventionFacility<'a> {
    fn invention_tax(&self) -> f64 {
        match &self.0.type_specific_data {
            FacilityType::Station(s) => s.invention_tax(),
            FacilityType::Structure(s) => s.invention_tax(),
        }
    }

    fn job_cost_modifier(&self) -> Option<f64> {
        match &self.0.type_specific_data {
            FacilityType::Station(s) => InventionFacilityProperties::job_cost_modifier(s),
            FacilityType::Structure(s) => InventionFacilityProperties::job_cost_modifier(s),
        }
    }

    fn job_duration_modifier(&self) -> Option<f64> {
        match &self.0.type_specific_data {
            FacilityType::Station(s) => InventionFacilityProperties::job_duration_modifier(s),
            FacilityType::Structure(s) => InventionFacilityProperties::job_duration_modifier(s),
        }
    }
}

impl<'a> Named for InventionFacility<'a> {
    fn name(&self) -> String {
        self.0.name()
    }
}

#[derive(Debug, Clone)]
pub struct JobInventionCostDetails {
    pub estimated_item_value: f64,
    pub base_job_cost: f64,
    pub job_cost_modifier: Option<f64>,
    pub facility_invention_tax: f64,
    pub scc_surcharge: f64,
}

#[derive(Debug)]
pub struct InventionResult {
    pub blueprint: InventionResultBlueprint,
    pub cost_normalized: f64,
    pub time_normalized: f64,
}

#[derive(Debug)]
pub struct InventionResultBlueprint {
    pub id: i32,
    pub runs: i32,
    pub material_efficiency: u8,
    pub time_efficiency: u8,
}

#[derive(Debug, Clone)]
pub struct InventionDetails {
    pub cost: CostsDetails,
    pub time: TimeDetails,
    pub success_probability: SuccessProbabilityDetails,
}

#[derive(Debug, Clone)]
pub struct CostsDetails {
    pub inputs: InputMaterialsCostsDetails,
    pub job_cost: JobInventionCostDetails,
    pub total: f64,
}

#[derive(Debug, Clone)]
pub struct TimeDetails {
    // base_time: i32,
    // job_duration_modifier: Option<f64>,
    // advanced_industry_skill_level: i32,
}

#[derive(Debug, Clone)]
pub struct SuccessProbabilityDetails {
    pub base_success_chance: f64,
    pub skills: Vec<TrainedSkill>,
    pub final_success_chance: f64,
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use crate::{
        api::evecache::mocks::MockRequester,
        model::{
            blueprint::{Materials, MultipleItems, ProbableMultipleItems},
            facility::{playerstructure::PlayerStructureStats, FacilityUsage},
            industry::IndustryType,
            items::{Item, TechLevel},
            locations::{Constellation, CostIndexes, Region, SolarSystem},
            prices::ItemPrice,
            skills::Skill,
        },
        round::Round,
    };

    use super::*;

    fn create_test_facility() -> Facility {
        let mut activites = HashMap::new();
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

    #[test]
    fn test_job_installation_cost() {
        let facility = create_test_facility();

        let estimated_item_value = 123.4; // Example estimated item value for testing

        let result = facility
            .invention()
            .unwrap()
            .job_installation_cost(estimated_item_value);

        assert_eq!(result.value.specific_round(2), -3.07);
        assert_eq!(result.details.estimated_item_value, estimated_item_value);
        assert_eq!(result.details.base_job_cost, 2.468);
        assert_eq!(result.details.job_cost_modifier, Some(3.0));
        assert_eq!(result.details.facility_invention_tax, 0.32);
        assert_eq!(result.details.scc_surcharge, 0.015);
    }

    #[test]
    fn test_time_per_run() {
        let facility = create_test_facility();

        let blueprint = BlueprintInvention {
            blueprint_id: 1,
            materials: Materials::new(vec![]),
            products: vec![ProbableMultipleItems {
                quantity: 2,
                base_probability: Some(0.3),
                item: Item::new(10, "test", None, TechLevel::One),
            }],
            skills: vec![
                Skill::new(10, "Skill1"),
                Skill::new(11, "Skill2"),
                Skill::new(12, "Skill3"),
            ],
            time: 100,
        };

        let skills = ManufacturingSkills {
            industry: Some(TrainedSkill::new(10, "Industry", 5)),
            advanced_industry_level: Some(TrainedSkill::new(10, "Advenced Industry", 2)),
        };

        let result = facility
            .invention()
            .unwrap()
            .time_per_run(&blueprint, &skills);

        assert_eq!(result.value, -235)
    }

    #[tokio::test]
    async fn test_success_probability() {
        let facility = create_test_facility();

        let blueprint = BlueprintInvention {
            blueprint_id: 1,
            materials: Materials::new(vec![]),
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
        };

        let skills = Skills {
            skills: vec![
                TrainedSkill::new(10, "Skill1", 3),
                TrainedSkill::new(11, "Skill2", 1),
                TrainedSkill::new(12, "Skill3 Encryption Methods", 5),
            ],
        };

        let result = facility
            .invention()
            .unwrap()
            .success_probability(&blueprint, 10, &skills)
            .await;

        assert_eq!(result.details.base_success_chance, 0.3);
        assert_eq!(result.details.final_success_chance, 0.3775);
        assert_eq!(result.value, 0.3775);
    }

    #[tokio::test]
    async fn test_invent_item() {
        let facility = create_test_facility();

        let blueprint = BlueprintManufacturing {
            blueprint_id: 10,
            materials: Materials::new(vec![]),
            products: vec![],
            material_efficiency: 0,
            time_efficiency: 0,
            time: 0,
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
        };

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
        let prices = Prices { prices };

        let result = facility
            .invention()
            .unwrap()
            .invent_item(&blueprint, &skills, &prices)
            .await;

        assert_eq!(result.details.cost.total, 32.0);
        assert_eq!(result.details.success_probability.base_success_chance, 0.3);
        assert_eq!(
            result.details.success_probability.final_success_chance,
            0.3775
        );
        assert_eq!(result.value.cost_normalized.specific_round(2), 84.77);
        assert_eq!(result.value.time_normalized.specific_round(2), -601.32);
        assert_eq!(result.value.blueprint.material_efficiency, 2);
        assert_eq!(result.value.blueprint.time_efficiency, 4);
        assert_eq!(result.value.blueprint.id, 10);
        assert_eq!(result.value.blueprint.runs, 2);
    }
}
