use chrono::Duration;
use colored::{ColoredString, Colorize};
use futures_util::{
    future::{try_join4, JoinAll, TryJoinAll},
    TryFutureExt,
};
use serde::Serialize;
use tokio::{join, try_join};

use crate::{
    configuration::cli::InventionItemOptions,
    display::Display,
    errors::{EveError, ModelError},
    integration::DataIntegrator,
    interactive,
    logging::{self, Message, Stdout, Verbosity},
    model::{
        blueprint::{
            InputMaterialsCostsDetails, MultipleItemsCostDetails, MultipleItemsOrdersDetails,
        },
        common::{DetailedCalculation, Identified, Named},
        facility::invention::InventionFacilityProperties,
        industry::IndustryType,
    },
    vector::UniqueElement,
};

pub async fn invention(eve: &DataIntegrator, opts: &InventionItemOptions) -> Result<(), EveError> {
    let item_to_invent =
        match interactive::load_item(eve, opts.item.clone(), opts.strict, None).await? {
            Some(item_to_invent) => item_to_invent,
            None => return Ok(()),
        };

    let manufacturing_blueprint = eve
        .load_item_blueprints(item_to_invent.id(), IndustryType::Manufacturing)
        .await?
        .unique(&format!(
            "Manufacturing blueprints: {}",
            item_to_invent.id()
        ))?;
    let (mut found_blueprints, character, facilities, prices) = try_join4(
        eve.load_item_blueprints(manufacturing_blueprint.id(), IndustryType::Invention)
            .map_err(|source| ModelError::LoadingBlueprint { source }),
        eve.load_character()
            .map_err(|source| ModelError::LoadingCharacter { source }),
        eve.load_registered_facilities()
            .map_err(|source| ModelError::LoadingFacilities { source }),
        eve.load_prices()
            .map_err(|source| ModelError::LoadingPrices { source }),
    )
    .await?;

    let invention_blueprint = found_blueprints.unique(&format!(
        "Invention blueprints: {}",
        manufacturing_blueprint.id()
    ))?;

    let invention_blueprint = match invention_blueprint.activities.invention {
        Some(blueprint_invention) => blueprint_invention,
        None => {
            return Err(ModelError::IsNotAnInventionBlueprint {
                blueprint_id: invention_blueprint.id(),
            })?;
        }
    };

    let mut invention_facilities = vec![];
    for facility in &facilities {
        if let Some(invention_facility) = facility.invention() {
            invention_facilities.push(invention_facility);
        }
    }

    let input_materials_costs = invention_blueprint
        .materials
        .input_materials_cost(None, None, &prices);
    let inputs_stdout = InputsStdout::from(&input_materials_costs);

    let estimated_item_value = manufacturing_blueprint
        .activities
        .manufacturing
        .as_ref()
        .expect("Requested as a manufacturing blueprint")
        .estimated_item_value(&prices);

    let success_probability= invention_blueprint
        .get_product(manufacturing_blueprint.id())
        .unwrap_or_else(|| panic!("Loaded invention blueprint as been loaded based on the product id (Blueprint ID:{};Invention Product ID{})",invention_blueprint.blueprint_id, manufacturing_blueprint.id()))
        .base_probability
        .unwrap_or(1.0);

    let manufacturing_skills = character.skills.get_manufacturing_skill();
    let mut advanced_industry_skill_level = 0;
    if let Some(advanced_industry_skill) = &manufacturing_skills.advanced_industry_level {
        advanced_industry_skill_level = advanced_industry_skill.trained_level;
    }

    let mut futures = vec![];
    for skill in &invention_blueprint.skills {
        let found_skill = character.skills.get_skill(skill.id());
        futures.push(async move {
            match found_skill {
                Some(s) => Ok::<(String, i32), ModelError>((s.name(), s.trained_level)),
                None => {
                    let skill_type = eve.api().get_type(skill.id()).await.map_err(|source| {
                        ModelError::LoadSkill {
                            skill_id: skill.id(),
                            source,
                        }
                    })?;
                    Ok((skill_type.name, 0))
                }
            }
        });
    }
    let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
    let skills = try_join!(all_futures)?.0;

    let mut all_science_skills = 0.0;
    let mut encryption_skills_level = 0.0;
    for (name, level) in &skills {
        if name.ends_with("Encryption Methods") {
            encryption_skills_level += *level as f64;
        } else {
            all_science_skills += *level as f64;
        }
    }

    let final_success_chance = success_probability
        * (1.0 + (all_science_skills / 30.0) + (encryption_skills_level / 40.0));

    let success_probability_stdout = SuccessProbabilityStdout {
        base_success_chance: success_probability,
        skills,
        final_success_chance,
    };

    let mut futures = vec![];
    for facility in invention_facilities {
        let invention_blueprint = invention_blueprint.clone();
        let manufacturing_skills = manufacturing_skills.clone();
        let input_stdout = inputs_stdout.clone();
        let success_probability_stdout = success_probability_stdout.clone();
        futures.push(async move {
            let installation_cost = facility.job_installation_cost(estimated_item_value).value;
            let total_run = installation_cost + input_materials_costs.value;
            let normalization_factor = 1.0 / final_success_chance;
            let total_cost_normalized = total_run * normalization_factor;
            let total_time_run = facility
                .time_per_run(&invention_blueprint, &manufacturing_skills)
                .value;
            let total_time_normalized = (total_time_run as f64) * normalization_factor;
            let total_time_normalized = total_time_normalized as i64;

            FacilityStdout {
                facility_name: facility.name(),
                costs: CostStdout {
                    inputs: input_stdout,
                    job_cost: JobCostStdout {
                        estimated_item_value,
                        system_cost_index: facility.facility().location.indexes.invention,
                        facility_tax: facility.invention_tax(),
                        job_cost_modifier: facility.job_cost_modifier(),
                        total: installation_cost,
                    },
                    total_run,
                },
                time: TimeStdout {
                    base_time: invention_blueprint.time,
                    job_duration_modifier: facility.job_duration_modifier(),
                    advanced_industry_skill_level,
                    total_time_run,
                },
                success_probability: success_probability_stdout,
                total_cost_normalized,
                total_time_normalized,
            }
        });
    }
    let all_futures = futures.into_iter().collect::<JoinAll<_>>();
    let facilities = join!(all_futures).0;

    logging::stdoutln(InventionStdout {
        searched_item_name: item_to_invent.name(),
        facilities,
    })?;
    Ok(())
}

#[derive(Serialize, Debug)]
struct InventionStdout {
    searched_item_name: String,
    facilities: Vec<FacilityStdout>,
}

impl Stdout for InventionStdout {}

impl Message for InventionStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let mut facilities = String::from("\tInvention facilities:\n");
        for facility in &self.facilities {
            facilities += facility.standard(verbosity).to_string().as_str();
        }
        ColoredString::from(
            format!("{}:\n\n{}", self.searched_item_name.bold(), facilities).as_str(),
        )
    }
}

#[derive(Serialize, Debug)]
struct FacilityStdout {
    facility_name: String,
    costs: CostStdout,
    time: TimeStdout,
    success_probability: SuccessProbabilityStdout,
    total_cost_normalized: f64,
    total_time_normalized: i64,
}

impl Message for FacilityStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let facility_name = self.facility_name.bold();
        let cost_str = self.costs.standard(verbosity);
        let time_str = self.time.standard(verbosity);
        let success_probability_str = self.success_probability.standard(verbosity);

        let total_time_normalized_str = format!(
            "\t\tTotal Time (Normalized): {:>61}",
            Duration::seconds(self.total_time_normalized).to_display()
        );
        let total_cost_normalized_str = format!(
            "\t\tTotal Cost (Normalized): {:>61} ISK",
            self.total_cost_normalized.to_display().bold()
        );

        ColoredString::from(
            format!("\t> {facility_name}:\n{cost_str}\n{time_str}\n{success_probability_str}\n{total_time_normalized_str}\n{total_cost_normalized_str}\n\n")
                .as_str(),
        )
    }
}

#[derive(Serialize, Debug)]
struct TimeStdout {
    base_time: i32,
    job_duration_modifier: Option<f64>,
    advanced_industry_skill_level: i32,
    total_time_run: i32,
}

impl Message for TimeStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        let base_time = format!(
            "\t\t\tBase time: {:>67}\n",
            Duration::seconds(self.base_time as i64).to_display()
        );
        let modifier = match self.job_duration_modifier {
            None => "".to_string(),
            Some(modifier) => {
                format!(
                    "\t\t\tDuration modifier: {:>59} %\n",
                    (modifier * 100.0).to_display()
                )
            }
        };
        let advanced_industry_level = format!(
            "\t\t\tAdvanced Industry skill: {:>21} * {:>10} = {:>16} %\n",
            self.advanced_industry_skill_level,
            "3 %",
            self.advanced_industry_skill_level * 3
        );
        let total = format!(
            "\t\t\tTime required per run: {:>55}\n",
            Duration::seconds(self.total_time_run as i64).to_display()
        );
        ColoredString::from(
            format!("\t\tDuration:\n{base_time}{modifier}{advanced_industry_level}{total}\n")
                .as_str(),
        )
    }
}

#[derive(Serialize, Debug)]
struct CostStdout {
    inputs: InputsStdout,
    job_cost: JobCostStdout,
    total_run: f64,
}

impl Message for CostStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let inputs = self.inputs.standard(verbosity);
        let job_cost = self.job_cost.standard(verbosity);

        let total_run_str = format!("{} ISK", self.total_run.to_display().underline());
        let total_run = format!("\t\t\tTotal (for one run): {:>69}", total_run_str);

        ColoredString::from(format!("\t\tCosts:\n{inputs}\n{job_cost}\n{total_run}\n").as_str())
    }
}

#[derive(Serialize, Debug, Clone)]
struct InputsStdout {
    inputs: Vec<InputStdout>,
    total: f64,
}

impl InputsStdout {
    pub fn from(
        materials_calculations: &DetailedCalculation<f64, InputMaterialsCostsDetails>,
    ) -> Self {
        let mut inputs = vec![];
        for cost in &materials_calculations.details.costs {
            inputs.push(InputStdout::from(cost))
        }
        InputsStdout {
            inputs,
            total: materials_calculations.value,
        }
    }
}

impl Message for InputsStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let first_line = "\t\t\tInput Materials:\n";

        let mut inputs_stdout = String::new();
        for input in &self.inputs {
            inputs_stdout += input.standard(verbosity).to_string().as_str();
        }

        let total_str = format!("{} ISK", self.total.to_display().underline());

        let total = format!("\t\t\tTotal of Input Materials: {:>64}\n", total_str);
        ColoredString::from(format!("{}{}\n{}", first_line, inputs_stdout, total).as_str())
    }
}

#[derive(Serialize, Debug, Clone)]
struct InputStdout {
    name: String,
    orders: Vec<OrderStdout>,
}

impl From<&MultipleItemsCostDetails> for InputStdout {
    fn from(details: &MultipleItemsCostDetails) -> Self {
        let mut orders = vec![];
        for order in &details.orders {
            orders.push(OrderStdout::from(order))
        }
        InputStdout {
            name: details.name.clone(),
            orders,
        }
    }
}

impl Message for InputStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let name_line = format!("\t\t\t\t{}:\n", self.name);
        let mut details_lines = String::new();
        for order in &self.orders {
            details_lines += order.standard(verbosity).to_string().as_str();
        }
        ColoredString::from(format!("{}{}", name_line, details_lines).as_str())
    }
}

#[derive(Clone, Serialize, Debug)]
struct OrderStdout {
    quantity: i32,
    price_per_unit: f64,
    total: f64,
}

impl Message for OrderStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        ColoredString::from(
            format!(
                "\t\t\t\t{:>20} * {:>20} ISK = {:>20} ISK\n",
                self.quantity.to_display(),
                self.price_per_unit.to_display(),
                self.total.to_display(),
            )
            .as_str(),
        )
    }
}

impl From<&MultipleItemsOrdersDetails> for OrderStdout {
    fn from(value: &MultipleItemsOrdersDetails) -> Self {
        OrderStdout {
            quantity: value.effective_quantity,
            price_per_unit: value.price_per_unit,
            total: value.total,
        }
    }
}

#[derive(Serialize, Debug)]
struct JobCostStdout {
    estimated_item_value: f64,
    system_cost_index: f64,
    facility_tax: f64,
    total: f64,
    job_cost_modifier: Option<f64>, // Also called structure bonus
}

impl Message for JobCostStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        let first_line = "\t\t\tJob Cost:\n";
        let system_cost_index = format!(
            "\t\t\t\tSystem cost index: {:>51} %\n",
            (self.system_cost_index * 100.0).to_display()
        );
        let estimated_item_value_line = format!(
            "\t\t\t\tEstimated item value: {:>48} ISK\n",
            self.estimated_item_value.to_display()
        );
        let job_cost_mod = match self.job_cost_modifier {
            None => String::new(),
            Some(modifier) => {
                format!(
                    "\t\t\t\tJob cost modifier: {:>51} %\n",
                    (modifier * 100.0).to_display()
                )
            }
        };
        let facility_tax = format!(
            "\t\t\t\tFacility Tax: {:>56} %\n",
            (self.facility_tax * 100.0).to_display()
        );

        let total_str = format!("{} ISK", self.total.to_display().underline());

        let total = format!("\t\t\tTotal for job installation cost: {:>57}\n", total_str);
        ColoredString::from(
            format!(
                "{}{}{}{}{}\n{}",
                first_line,
                estimated_item_value_line,
                system_cost_index,
                job_cost_mod,
                facility_tax,
                total
            )
            .as_str(),
        )
    }
}

#[derive(Clone, Serialize, Debug)]
struct SuccessProbabilityStdout {
    base_success_chance: f64,
    skills: Vec<(String, i32)>,
    final_success_chance: f64,
}

impl Message for SuccessProbabilityStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        let base_success_probability_str = format!(
            "\t\t\tBase success probability: {:>51} %",
            self.base_success_chance.to_display()
        );
        let final_success_probability_str = format!(
            "\t\t\tFinal success probability: {:>50} %",
            self.final_success_chance.to_display()
        );

        let mut skills_str = String::from("\t\t\tSkills:\n");
        for (skill_name, level) in &self.skills {
            skills_str += format!("\t\t\t\t{:>30}{:>39}\n", skill_name, level).as_str();
        }

        ColoredString::from(
            format!("\t\tSuccess Probabilities:\n{base_success_probability_str}\n{skills_str}\n{final_success_probability_str}\n").as_str(),
        )
    }
}
