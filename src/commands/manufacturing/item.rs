use chrono::Duration;
use colored::{ColoredString, Colorize};
use futures_util::future::{try_join, try_join4, JoinAll, TryJoinAll};
use futures_util::TryFutureExt;
use serde::Serialize;
use tokio::{join, try_join};

use crate::configuration::cli::{ManufacturingOptions, MultipleItemsOptions};
use crate::dates::NaivePeriod;
use crate::display::Display;
use crate::errors::{EveError, ModelError};
use crate::integration::DataIntegrator;
use crate::logging::{Message, Stdout, Verbosity};
use crate::model::blueprint::{
    BlueprintManufacturing, InputMaterialsCostsDetails, MultipleItemsCostDetails,
    MultipleItemsOrdersDetails,
};
use crate::model::common::{DetailedCalculation, Identified, Named};
use crate::model::facility::manufacture::ManufacturingFacility;
use crate::model::facility::markets::{Market, OrdersRange};
use crate::model::industry::IndustryType;
use crate::model::items::TechLevel;
use crate::{interactive, logging};

pub async fn manufacture(
    eve: &DataIntegrator,
    opts_manufacturing: &ManufacturingOptions,
    opts: &MultipleItemsOptions,
) -> Result<(), EveError> {
    logging::debug!("{:?}", opts_manufacturing);
    let item_name = opts.item.clone();
    let item_to_manufacture =
        match interactive::load_item(eve, item_name, opts.strict, None).await? {
            Some(item_to_manufacture) => item_to_manufacture,
            None => return Ok(()),
        };

    let (found_blueprints, character, facilities, prices) = try_join4(
        eve.load_item_blueprints(item_to_manufacture.id(), IndustryType::Manufacturing)
            .map_err(|source| ModelError::LoadingBlueprint { source }),
        eve.load_character()
            .map_err(|source| ModelError::LoadingCharacter { source }),
        eve.load_registered_facilities()
            .map_err(|source| ModelError::LoadingFacilities { source }),
        eve.load_prices()
            .map_err(|source| ModelError::LoadingPrices { source }),
    )
    .await?;

    let mut found_blueprints: Vec<BlueprintManufacturing> = found_blueprints
        .iter()
        .filter_map(|b| b.activities.manufacturing.clone())
        .collect();
    if found_blueprints.is_empty() {
        return Err(ModelError::BlueprintMissing {
            name: item_to_manufacture.name(),
            type_id: item_to_manufacture.id(),
        })?;
    } else if found_blueprints.len() > 1 {
        return Err(ModelError::TooMuchBlueprint {
            name: item_to_manufacture.name(),
            type_id: item_to_manufacture.id(),
        })?;
    }
    let mut blueprint = found_blueprints.remove(0);
    blueprint.material_efficiency = opts_manufacturing.material_efficiency;
    blueprint.time_efficiency = opts_manufacturing.time_efficiency;

    let mut manufactures = vec![];
    let mut inventions = vec![];
    let mut markets = vec![];
    for facility in &facilities {
        if let Some(manufacture) = facility.manufacture() {
            manufactures.push(manufacture);
        }
        if let Some(invention) = facility.invention() {
            inventions.push(invention);
        }
        if let Some(market) = facility.market() {
            markets.push(market);
        }
    }

    let mut blueprint_run_price = 0.0;
    let mut invention_stdout = None;
    if TechLevel::Two == item_to_manufacture.tech_level {
        let mut futures = vec![];
        for invention in &inventions {
            futures.push(invention.invent_item(&blueprint, &character.skills, &prices));
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

            invention_stdout = Some(InventionStdout {
                base_cost_run: res.details.cost.total,
                success_chance: res.details.success_probability.final_success_chance,
                runs: res.value.blueprint.runs,
                total_run: total_per_run,
            });

            blueprint.material_efficiency = res.value.blueprint.material_efficiency;
            blueprint.time_efficiency = res.value.blueprint.time_efficiency;

            blueprint_run_price = total_per_run;
        }
    }

    let manufacturing_skills = character.skills.get_manufacturing_skill();

    let item_id = item_to_manufacture.id();
    let mut futures = vec![];
    for market in &markets {
        futures.push(async move {
            let lowest_sell_price = market
                .lowest_sell_price(item_id, OrdersRange::Station)
                .await?;
            let market_stdout = MarketStdout::from(market, item_id).await?;
            Ok::<(MarketStdout, Option<f64>), EveError>((market_stdout, lowest_sell_price))
        })
    }
    let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
    let markets_data = try_join!(all_futures)?.0;

    let mut markets_stdout = vec![];
    let mut markets_sell_prices = vec![];
    for (market_stdout, lowest_sell_price) in markets_data {
        markets_sell_prices.push((market_stdout.name.clone(), lowest_sell_price));
        markets_stdout.push(market_stdout);
    }

    let estimated_item_value = blueprint.estimated_item_value(&prices);
    let output_quantity = blueprint.get_product(item_to_manufacture.id())
        .unwrap_or_else(|| panic!("Product should exist because Blueprint ({}) hase been loaded based on it's ID ({})", blueprint.blueprint_id, item_to_manufacture.id()))
        .quantity;

    let mut futures = vec![];
    for manufacture in manufactures {
        let blueprint = &blueprint;
        let prices = &prices;
        let manufacturing_skills = &manufacturing_skills;
        let markets_sell_prices = &markets_sell_prices;
        futures.push(async move {
            let input_material_cost = blueprint.materials.input_materials_cost(
                manufacture.material_consumption_modifier(),
                Some(blueprint.material_efficiency),
                prices,
            );
            let inputs = InputsStdout::from(&input_material_cost);
            let system = &manufacture.facility().location;
            let job_installation_cost = manufacture.job_installation_cost(estimated_item_value);
            let total_run = input_material_cost.value + job_installation_cost + blueprint_run_price;
            let total_per_unit = total_run / (output_quantity as f64);
            let time_per_run = manufacture.time_per_run(blueprint, manufacturing_skills);
            let number_of_run_per_hour = 3600.0 / (time_per_run as f64);
            let time_per_unit = time_per_run / output_quantity;
            let unit_per_hour = 3600.0 / (time_per_unit as f64);

            let mut industry_skill_level = 0;
            if let Some(skill) = &manufacturing_skills.industry {
                industry_skill_level = skill.trained_level
            }
            let mut advanced_industry_skill_level = 0;
            if let Some(skill) = &manufacturing_skills.industry {
                advanced_industry_skill_level = skill.trained_level
            }

            let mut volume_stdout = None;
            if let Some(vol) = item_to_manufacture.volume {
                let per_hour_volume = vol * unit_per_hour;
                volume_stdout = Some(VolumeStdout {
                    per_unit_volume: vol,
                    per_run_volume: vol * (output_quantity as f64),
                    per_hour_volume,
                    per_day_volume: per_hour_volume * 24.0,
                });
            }

            let mut profits = vec![];
            for (name, sell_price) in markets_sell_prices.clone() {
                match sell_price {
                    None => profits.push(MarketProfitStdout {
                        name,
                        profit_per_hour: None,
                        profit_per_day: None,
                    }),
                    Some(p) => {
                        let margin = p - total_per_unit;
                        let profits_per_hour = margin * unit_per_hour;
                        profits.push(MarketProfitStdout {
                            name,
                            profit_per_hour: Some(profits_per_hour),
                            profit_per_day: Some(profits_per_hour * 24.0),
                        })
                    }
                }
            }

            FacilityStdout {
                facility_name: manufacture.name(),
                costs: CostStdout {
                    inputs,
                    job_cost: JobCostStdout {
                        estimated_item_value,
                        system_cost_index: system.indexes.manufacturing,
                        facility_tax: manufacture.manufacturing_tax(),
                        job_cost_modifier: manufacture.job_cost_modifier(),
                        total: job_installation_cost,
                    },
                    total_run,
                    total_per_unit,
                },
                time: TimeStdout {
                    base_time: blueprint.time,
                    job_duration_modifier: manufacture.job_duration_modifier(),
                    industry_skill_level,
                    advanced_industry_skill_level,
                    total_time_run: time_per_run,
                    number_of_run_per_hour,
                    number_of_run_per_day: number_of_run_per_hour * 24.0,
                },
                volume: volume_stdout,
                profit: ProfitsStdout {
                    markets_profit: profits,
                },
            }
        })
    }
    let all_futures = futures.into_iter().collect::<JoinAll<_>>();
    let facilities = join!(all_futures).0;

    logging::stdoutln(ManufactureStdout {
        searched_item_name: item_to_manufacture.name(),
        markets: markets_stdout,
        invention: invention_stdout,
        facilities,
    })?;
    Ok(())
}

#[derive(Serialize, Debug)]
struct ManufactureStdout {
    searched_item_name: String,
    markets: Vec<MarketStdout>,
    invention: Option<InventionStdout>,
    facilities: Vec<FacilityStdout>,
}

impl Stdout for ManufactureStdout {}

impl Message for ManufactureStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let mut markets = format!(
            "\tMarkets:\n\t{:>68}{:>32}\n",
            "Average Quantity Sold (30 days)".underline(),
            "Lowest sell price".underline()
        );
        for market in &self.markets {
            markets += market.standard(verbosity).to_string().as_str();
        }

        let mut invention_stdout = ColoredString::from("");
        if let Some(inv) = &self.invention {
            invention_stdout = inv.standard(verbosity);
        }

        let mut facilities = String::from("\tManufacturing facilities:\n");
        for facility in &self.facilities {
            facilities += facility.standard(verbosity).to_string().as_str();
        }
        ColoredString::from(
            format!(
                "{}:\n\n{}\n{}\n\n{}",
                self.searched_item_name.bold(),
                markets,
                invention_stdout,
                facilities
            )
            .as_str(),
        )
    }
}

#[derive(Serialize, Debug)]
struct MarketStdout {
    name: String,
    regional_average_volumes: i64,
    lowest_price: Option<f64>,
}

impl MarketStdout {
    async fn from<'a>(market: &Market<'a>, item_id: i32) -> Result<Self, EveError> {
        let (price, volume) = try_join(
            market
                .lowest_sell_price(item_id, OrdersRange::Station)
                .map_err(EveError::MarketError),
            market
                .regional_average_volume(item_id, NaivePeriod::past(Duration::days(30)))
                .map_err(EveError::VolumesError),
        )
        .await?;
        Ok(MarketStdout {
            name: market.name(),
            regional_average_volumes: volume,
            lowest_price: price,
        })
    }
}

impl Message for MarketStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        let (lowest_price_str, unit) = match self.lowest_price {
            None => (String::from("N/A"), ""),
            Some(lowest_price) => (lowest_price.to_display(), "ISK/u"),
        };
        ColoredString::from(
            format!(
                "{:>60} {:>15} {:>25} {}\n",
                self.name.bold(),
                self.regional_average_volumes.to_display(),
                lowest_price_str,
                unit
            )
            .as_str(),
        )
    }
}

#[derive(Serialize, Debug)]
struct FacilityStdout {
    facility_name: String,
    costs: CostStdout,
    time: TimeStdout,
    volume: Option<VolumeStdout>,
    profit: ProfitsStdout,
}

impl Message for FacilityStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let facility_name = self.facility_name.bold();
        let cost_str = self.costs.standard(verbosity);
        let time_str = self.time.standard(verbosity);
        let volume = match &self.volume {
            None => "".to_string(),
            Some(volume) => {
                format!("{}\n", volume.standard(verbosity))
            }
        };
        let profits_str = self.profit.standard(verbosity);
        ColoredString::from(
            format!("\t> {facility_name}:\n{cost_str}\n{time_str}\n{volume}{profits_str}\n\n")
                .as_str(),
        )
    }
}

#[derive(Serialize, Debug)]
struct InventionStdout {
    base_cost_run: f64,
    success_chance: f64,
    runs: i32,
    total_run: f64,
}

impl Message for InventionStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        ColoredString::from(
            format!(
                "\tInvention:\n\t\t{:>20} ISK * {:>10} %  / {:>10} = {:>30} ISK/run\n",
                self.base_cost_run.to_display(),
                (self.success_chance * 100.0).to_display(),
                self.runs,
                self.total_run.to_display()
            )
            .as_str(),
        )
    }
}

#[derive(Serialize, Debug)]
struct CostStdout {
    inputs: InputsStdout,
    job_cost: JobCostStdout,
    total_run: f64,
    total_per_unit: f64,
}

impl Message for CostStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let inputs = self.inputs.standard(verbosity);
        let job_cost = self.job_cost.standard(verbosity);

        let total_run_str = format!("{} ISK", self.total_run.to_display().underline());
        let total_run = format!("\t\t\tTotal (for one run): {:>69}", total_run_str);

        let total_per_unit_str = format!("{} ISK/u", self.total_per_unit.to_display().underline());
        let total_unit = format!("\t\t\tTotal (per unit): {:>74}", total_per_unit_str).bold();

        ColoredString::from(
            format!("\t\tCosts:\n{inputs}\n{job_cost}\n{total_run}\n{total_unit}\n").as_str(),
        )
    }
}

#[derive(Serialize, Debug)]
struct TimeStdout {
    base_time: i32,
    job_duration_modifier: Option<f64>,
    industry_skill_level: i32,
    advanced_industry_skill_level: i32,
    total_time_run: i32,
    number_of_run_per_hour: f64,
    number_of_run_per_day: f64,
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
        let industry_lvl = format!(
            "\t\t\tIndustry skill: {:>30} * {:>10} = {:>16} %\n",
            self.industry_skill_level,
            "4 %",
            self.industry_skill_level * 4
        );
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
        let run_per_hour = format!(
            "\t\t\tNumber of runs (Per Hour): {:>51}\n",
            self.number_of_run_per_hour.to_display()
        );
        let run_per_day = format!(
            "\t\t\tNumber of runs (Per Days): {:>51}\n",
            self.number_of_run_per_day.to_display()
        );
        ColoredString::from(format!("\t\tDuration:\n{base_time}{modifier}{industry_lvl}{advanced_industry_level}{total}\n{run_per_hour}{run_per_day}\n").as_str())
    }
}

#[derive(Serialize, Debug)]
struct VolumeStdout {
    per_unit_volume: f64,
    per_run_volume: f64,
    per_hour_volume: f64,
    per_day_volume: f64,
}

impl Message for VolumeStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        let per_unit = format!(
            "\t\t\tPer Unit: {:>69} m3/u\n",
            self.per_unit_volume.to_display()
        );
        let per_run = format!(
            "\t\t\tPer Run: {:>70} m3\n",
            self.per_run_volume.to_display()
        );
        let per_hour = format!(
            "\t\t\tPer Hour: {:>69} m3/h\n",
            self.per_hour_volume.to_display()
        );
        let per_day = format!(
            "\t\t\tPer Day: {:>70} m3/d\n",
            self.per_day_volume.to_display()
        );
        ColoredString::from(
            format!("\t\tVolumes: \n{per_unit}{per_run}{per_hour}{per_day}\n").as_str(),
        )
    }
}

#[derive(Serialize, Debug)]
struct ProfitsStdout {
    markets_profit: Vec<MarketProfitStdout>,
}

impl Message for ProfitsStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let mut profits_str = String::new();
        for profit in &self.markets_profit {
            profits_str += profit.standard(verbosity).to_string().as_str();
        }
        ColoredString::from(format!("\t\tProfits:\n{profits_str}\n").as_str())
    }
}

#[derive(Serialize, Debug)]
struct MarketProfitStdout {
    name: String,
    profit_per_hour: Option<f64>,
    profit_per_day: Option<f64>,
}

impl Message for MarketProfitStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        let pph_str = match self.profit_per_hour {
            None => "".to_string(),
            Some(pph) => pph.to_display(),
        };
        let ppd_str = match self.profit_per_day {
            None => "".to_string(),
            Some(ppd) => ppd.to_display(),
        };

        ColoredString::from(
            format!(
                "\t\t\t{:>48}{:>30} ISK/h\n{:>102} ISK/d\n",
                self.name, pph_str, ppd_str
            )
            .as_str(),
        )
    }
}

#[derive(Serialize, Debug)]
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

#[derive(Serialize, Debug)]
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

#[derive(Serialize, Debug)]
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
