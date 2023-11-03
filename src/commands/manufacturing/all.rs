use std::collections::HashSet;

use chrono::Duration;
use colored::{ColoredString, Colorize};
use futures_util::future::{try_join3, TryJoinAll};
use futures_util::TryFutureExt;
use serde::Serialize;
use tokio::try_join;

use crate::api::evecache::cache_keys::OrderType;
use crate::configuration::cli::{ManufactureAllOptions, ManufacturingOptions};
use crate::dates::NaivePeriod;
use crate::display::Display;
use crate::errors::{EveError, ModelError};
use crate::integration::DataIntegrator;
use crate::logging;
use crate::logging::{Message, Stdout, Verbosity};
use crate::model::blueprint::BlueprintManufacturing;
use crate::model::character::Character;
use crate::model::common::{Identified, Named};
use crate::model::facility::invention::InventionFacility;
use crate::model::facility::manufacture::{Manufacture, ManufactureError};
use crate::model::facility::markets::{Market, RegionOrders};
use crate::model::industry::IndustryType;
use crate::model::items::Item;
use crate::model::prices::Prices;

pub async fn manufacture_all(
    eve: &DataIntegrator,
    opts_manufacturing: &ManufacturingOptions,
    opts: &ManufactureAllOptions,
) -> Result<(), EveError> {
    let (character, facilities, prices) = try_join3(
        eve.load_character()
            .map_err(|source| ModelError::LoadingCharacter { source }),
        eve.load_registered_facilities()
            .map_err(|source| ModelError::LoadingFacilities { source }),
        eve.load_prices()
            .map_err(|source| ModelError::LoadingPrices { source }),
    )
    .await?;

    let items = if opts.everything {
        eve.load_all_items_with_blueprint(IndustryType::Manufacturing)
            .await?
    } else {
        eve.load_registered_items().await?
    };

    logging::info!("Data retrieved - Compute {} items", items.len());

    let mut manufactures = vec![];
    let mut invention_facilities = vec![];
    let mut markets = vec![];
    for facility in &facilities {
        if let Some(manufacture) = facility.manufacture() {
            manufactures.push(manufacture);
        }
        if let Some(invention) = facility.invention() {
            invention_facilities.push(invention);
        }
        if let Some(market) = facility.market() {
            markets.push(market);
        }
    }

    let mut region_ids = HashSet::new();
    for market in &markets {
        region_ids.insert(market.as_factility().location.constellation.region.id());
    }

    let mut futures = vec![];
    for region_id in region_ids {
        futures.push(eve.load_market_orders(region_id, OrderType::Sell));
    }
    let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
    let orders = try_join!(all_futures)?.0;

    logging::info!("Compute items manufacturing");
    let mut futures = vec![];
    for item in items {
        let prices = prices.clone();
        futures.push(async {
            let found_blueprints = eve
                .load_item_blueprints(item.id(), IndustryType::Manufacturing)
                .await?;
            let mut found_blueprints: Vec<BlueprintManufacturing> = found_blueprints
                .into_iter()
                .filter_map(|b| b.activities.manufacturing)
                .collect();
            if found_blueprints.is_empty() {
                return Err(ModelError::BlueprintMissing {
                    name: item.name(),
                    type_id: item.id(),
                })?;
            }

            let mut blueprint = found_blueprints.remove(0);
            blueprint.material_efficiency = opts_manufacturing.material_efficiency;
            blueprint.time_efficiency = opts_manufacturing.time_efficiency;
            load_item(
                item,
                blueprint,
                Facilities {
                    manufactures: &manufactures,
                    invention_facilities: &invention_facilities,
                    markets: &markets,
                },
                &orders,
                &character,
                prices,
                opts.everything,
            )
            .await
        });
    }
    let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
    let mut item_sdout = try_join!(all_futures)?.0;
    item_sdout.sort_by(|a, b| b.profits_per_hour.partial_cmp(&a.profits_per_hour).unwrap());
    logging::stdoutln(ManufactureAllStdout { items: item_sdout })?;
    Ok(())
}

struct Facilities<'a> {
    manufactures: &'a Vec<Manufacture<'a>>,
    invention_facilities: &'a Vec<InventionFacility<'a>>,
    markets: &'a Vec<Market<'a>>,
}

async fn load_item<'a>(
    item: Item,
    blueprint: BlueprintManufacturing,
    facilities: Facilities<'a>,
    orders: &[RegionOrders],
    character: &Character,
    prices: Prices,
    everything: bool,
) -> Result<ItemStdout, EveError> {
    let output_quantity = blueprint.get_product(item.id())
        .unwrap_or_else(|| panic!("Product should exist because Blueprint ({}) hase been loaded based on it's ID ({})", blueprint.blueprint_id, item.id()))
        .quantity;

    let mut futures = vec![];
    for market in facilities.markets {
        let item_id = item.id();
        futures.push(async move {
            let orders = orders.iter().find(|&order| {
                order.region.id() == market.as_factility().location.constellation.region.id()
            });
            let lowest_sell_price = orders.map(|orders| {
                orders.lowest_station_sell_price(item_id, market.as_factility().id())
            });
            let mut regional_average_volume = None;
            if !everything {
                regional_average_volume = Some(
                    market
                        .regional_average_volume(item_id, NaivePeriod::past(Duration::days(30)))
                        .await?,
                );
            }
            Ok::<(Option<f64>, Option<i64>), EveError>((lowest_sell_price, regional_average_volume))
        })
    }
    let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
    let markets_datas = try_join!(all_futures)?.0;

    let mut highest_sell_price = 0.0;
    let mut regional_average_volume = None;
    for market_data in markets_datas {
        if let Some(price) = market_data.0 {
            if highest_sell_price == 0.0 || highest_sell_price < price {
                highest_sell_price = price;
                regional_average_volume = market_data.1;
            }
        }
    }

    let mut futures = vec![];
    for manufacture in facilities.manufactures {
        futures.push(async {
            let cost_per_run = manufacture
                .manufacture_cost_per_run(
                    &blueprint,
                    &character.skills,
                    facilities.invention_facilities,
                    &prices,
                )
                .await?;
            let cost_per_unit = cost_per_run / (output_quantity as f64);
            let margin = highest_sell_price - cost_per_unit;
            let time_per_run =
                manufacture.time_per_run(&blueprint, &character.skills.get_manufacturing_skill());
            let time_per_unit = time_per_run / output_quantity;
            let unit_per_hour = 3600.0 / (time_per_unit as f64);
            let profits_per_hour = margin * unit_per_hour;
            Ok::<f64, ManufactureError>(profits_per_hour)
        });
    }
    let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
    let profits = try_join!(all_futures)?.0;

    let mut max_profit = 0.0;
    for profit in profits {
        if max_profit == 0.0 || max_profit < profit {
            max_profit = profit;
        }
    }
    logging::debug!("Computed manufacturing of item: {}", item.name());
    Ok(ItemStdout {
        name: item.name(),
        profits_per_hour: max_profit,
        regional_average_volume,
    })
}

#[derive(Serialize)]
pub struct ManufactureAllStdout {
    items: Vec<ItemStdout>,
}

impl Stdout for ManufactureAllStdout {}

impl Message for ManufactureAllStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let mut items_str = String::new();
        for item in &self.items {
            items_str += item.standard(verbosity).to_string().as_str();
        }
        ColoredString::from(items_str.as_str())
    }
}

#[derive(Serialize)]
pub struct ItemStdout {
    name: String,
    regional_average_volume: Option<i64>,
    profits_per_hour: f64,
}

impl Message for ItemStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        let volume_str = match self.regional_average_volume {
            Some(vol) => vol.to_display(),
            None => "".to_string(),
        };
        ColoredString::from(
            format!(
                "{:>50}{:>20}{:>50} ISK/h\n",
                self.name.bold(),
                volume_str,
                self.profits_per_hour.to_display()
            )
            .as_str(),
        )
    }
}
