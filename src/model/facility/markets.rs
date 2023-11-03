use std::fmt::Display;

use chrono::{NaiveDate, ParseError};
use rfesi::{groups::MarketOrder, prelude::EsiError};
use thiserror::Error;

use crate::{
    api::evecache::cache::CacheError,
    dates::NaivePeriod,
    model::{
        common::{Identified, Named},
        locations::Region,
    },
    retry::{retry, RetryableError},
};

use super::{Facility, IdentifierTypeConversionFailed};

#[derive(Debug, Error)]
pub enum MarketError {
    // #[error(
    //     "Could not load fulfill order for {item_name} (missing:{missing};requested:{requested})"
    // )]
    // CouldNotFulfillOrder {
    //     item_name: String,
    //     missing: i32,
    //     requested: i32,
    // },
    #[error("Could not load order for id '{item_id}' in range {range}: {source}")]
    CouldNotLoadOrders {
        item_id: i32,
        range: OrdersRange,
        source: APIError,
    },
    #[error("Could not load facility: {source}")]
    CouldNotLoadFacility {
        #[from]
        source: IdentifierTypeConversionFailed,
    },
}

#[derive(Debug, Error)]
pub enum VolumesError {
    #[error("Could not load order history for id '{item_id}' in region '{region_id}': {source}")]
    CouldNotLoadOrdersHistory {
        item_id: i32,
        region_id: i32,
        source: APIError,
    },
    #[error("Could not parse history date ({date}) for item '{item_id}': {source}")]
    CouldNotParseHistoryDate {
        item_id: i32,
        date: String,
        source: ParseError,
    },
}

#[derive(Debug, Error)]
#[error(transparent)]
pub struct APIError {
    source: CacheError,
}

impl RetryableError for APIError {
    fn retryable(&self) -> bool {
        if let CacheError::Api { source } = &self.source {
            if let EsiError::ReqwestError(source) = &source.source {
                if source.is_timeout() {
                    return true;
                }
            }
        }
        false
    }
}

pub struct Market<'a>(&'a Facility);

impl<'a> Market<'a> {
    pub fn new(facility: &'a Facility) -> Self {
        Self(facility)
    }

    // pub async fn order(
    //     &self,
    //     item: &Item,
    //     quantity: i32,
    //     range: OrdersRange,
    // ) -> Result<Vec<Order>, MarketError> {
    //     let orders = self.sell_orders_by_price(item.id(), range).await?;

    //     let mut planned_orders = vec![];
    //     let mut fulfilled = quantity;
    //     for order in orders {
    //         let quantity = if fulfilled > order.volume_remain {
    //             order.volume_remain
    //         } else {
    //             fulfilled
    //         };
    //         planned_orders.push(Order {
    //             quantity,
    //             price: order.price,
    //         });
    //         fulfilled -= quantity;
    //         if fulfilled <= 0 {
    //             break;
    //         }
    //     }
    //     if fulfilled > 0 {
    //         return Err(MarketError::CouldNotFulfillOrder {
    //             item_name: item.name(),
    //             missing: fulfilled,
    //             requested: quantity,
    //         });
    //     }
    //     Ok(planned_orders)
    // }

    // pub async fn sell_orders_by_price(
    //     &self,
    //     item_id: i32,
    //     range: OrdersRange,
    // ) -> Result<Vec<MarketOrder>, MarketError> {
    //     let mut orders = retry(5, || async {
    //         self.0
    //             .eve
    //             .api()
    //             .esi()
    //             .group_market()
    //             .get_region_orders(
    //                 self.0.location.constellation.region.id(),
    //                 Some("sell".to_string()),
    //                 None,
    //                 Some(item_id),
    //             )
    //             .await
    //             .map_err(|source| APIError { source })
    //     })
    //     .await
    //     .map_err(|source| MarketError::CouldNotLoadOrders {
    //         item_id,
    //         range: range.clone(),
    //         source,
    //     })?;
    //     orders.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
    //     match range {
    //         OrdersRange::Region => Ok(orders),
    //         OrdersRange::Station => {
    //             let facility_id = self.0.try_id()?;
    //             Ok(orders
    //                 .into_iter()
    //                 .filter(|o| o.location_id == (facility_id as i64))
    //                 .collect())
    //         }
    //     }
    // }

    // async fn load_region_orders(&self, item_id: i32) -> Result<Vec<MarketOrder>, APIError> {
    //     let orders = self
    //         .0
    //         .eve
    //         .api()
    //         .esi()
    //         .group_market()
    //         .get_region_orders(
    //             self.0.location.constellation.region.id(),
    //             Some("sell".to_string()),
    //             None,
    //             Some(item_id),
    //         )
    //         .await;
    //     match orders {
    //         Ok(orders) => Ok(orders),
    //         Err(source) => Err(APIError { source }),
    //     }
    // }

    pub async fn lowest_sell_price(
        &self,
        item_id: i32,
        range: OrdersRange,
    ) -> Result<Option<f64>, MarketError> {
        let mut orders = retry(5, std::time::Duration::from_secs(1), || async {
            self.0
                .eve
                .get_region_orders(
                    self.0.location.constellation.region.id(),
                    Some("sell".to_string()),
                    None,
                    Some(item_id),
                )
                .await
                .map_err(|source| APIError { source })
        })
        .await
        .map_err(|source| MarketError::CouldNotLoadOrders {
            item_id,
            range: range.clone(),
            source,
        })?;

        orders.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
        if range == OrdersRange::Station {
            let facility_id = self.0.id();
            orders.retain(|o| o.location_id == facility_id);
        }
        let mut lowest_sell_order_price = 0.0;
        for order in orders {
            if lowest_sell_order_price == 0.0 || lowest_sell_order_price > order.price {
                lowest_sell_order_price = order.price;
            }
        }
        if lowest_sell_order_price == 0.0 {
            return Ok(None);
        }
        Ok(Some(lowest_sell_order_price))
    }

    pub async fn regional_average_volume(
        &self,
        item_id: i32,
        average_period: NaivePeriod,
    ) -> Result<i64, VolumesError> {
        let history = retry(5, std::time::Duration::from_secs(1), || async {
            self.0
                .eve
                .get_region_market_history(self.0.location.constellation.region.id(), item_id)
                .await
                .map_err(|source| APIError { source })
        })
        .await
        .map_err(|source| VolumesError::CouldNotLoadOrdersHistory {
            item_id,
            region_id: self.0.location.constellation.region.id(),
            source,
        })?;
        let mut truncated_history = vec![];
        for h in history {
            let date = h.date.parse::<NaiveDate>().map_err(|source| {
                VolumesError::CouldNotParseHistoryDate {
                    item_id,
                    date: h.date.clone(),
                    source,
                }
            })?;
            if average_period.contains_date(date) {
                truncated_history.push(h)
            }
        }
        let volume = truncated_history
            .iter()
            .map(|h| h.volume)
            .reduce(|acc, volume| acc + volume);

        match volume {
            None => Ok(0),
            Some(volume) => Ok(volume / (truncated_history.len() as i64)),
        }
    }

    pub fn as_factility(&self) -> &Facility {
        self.0
    }
}

impl<'a> Named for Market<'a> {
    fn name(&self) -> String {
        self.0.name()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum OrdersRange {
    // Region,
    Station,
}

impl Display for OrdersRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            OrdersRange::Station => "Station",
        };
        write!(f, "{}", str)
    }
}

#[derive(Debug)]
pub struct RegionOrders {
    pub region: Region,
    pub orders: Vec<MarketOrder>,
}

impl RegionOrders {
    // pub fn lowest_regional_sell_price(&self, type_id: i32) -> f64 {
    //     self.orders_for(type_id, OrdersRange::Region, None)
    //         .iter()
    //         .map(|s| s.price)
    //         .reduce(|acc, f| {
    //             if acc > f {
    //                 return f;
    //             } else {
    //                 return acc;
    //             }
    //         })
    //         .unwrap_or(0.0)
    // }

    pub fn lowest_station_sell_price(&self, type_id: i32, facility_id: i64) -> f64 {
        self.orders_for(type_id, OrdersRange::Station, Some(facility_id))
            .iter()
            .filter(|s| !s.is_buy_order)
            .map(|s| s.price)
            .reduce(|acc, f| if acc > f { f } else { acc })
            .unwrap_or(0.0)
    }

    fn orders_for(
        &self,
        type_id: i32,
        range: OrdersRange,
        facility_id: Option<i64>,
    ) -> Vec<&MarketOrder> {
        self.orders
            .iter()
            .filter(|o| o.type_id == type_id)
            .filter(|o| match range {
                // OrdersRange::Region => return true,
                OrdersRange::Station => o.location_id == facility_id.unwrap(),
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use rfesi::groups::HistoryItem;

    use crate::{
        api::evecache::{cache_keys::OrderType, mocks::MockRequester},
        model::{
            facility::FacilityUsage,
            locations::{Constellation, CostIndexes, SolarSystem},
        },
    };

    use super::*;

    fn create_facility(requester: Arc<MockRequester>) -> Facility {
        Facility::new_station(
            requester,
            8,
            "Test Station Name".to_string(),
            SolarSystem::new(
                1,
                "Test Solar System".to_string(),
                0.1234,
                vec![8],
                Constellation::new(
                    2,
                    "Test Constellation".to_string(),
                    vec![9],
                    Region::new(3, "Test Region", vec![10]),
                ),
                CostIndexes {
                    manufacturing: 0.456,
                    invention: 0.789,
                },
            ),
            Some(vec![FacilityUsage::Market, FacilityUsage::Industry]),
        )
    }

    fn create_market_order(
        id: i64,
        item_id: i32,
        facility_id: i64,
        order_type: OrderType,
        price: f64,
    ) -> MarketOrder {
        let ignored_number = 0;
        let ignored_string = "Ignored".to_string();

        MarketOrder {
            duration: ignored_number,
            is_buy_order: OrderType::Buy == order_type,
            issued: ignored_string.clone(),
            location_id: facility_id,
            min_volume: ignored_number,
            order_id: id,
            price,
            range: ignored_string.to_string(),
            system_id: ignored_number,
            type_id: item_id,
            volume_remain: ignored_number,
            volume_total: ignored_number,
        }
    }

    fn create_history_item(date: &str, volume: i64) -> HistoryItem {
        let ignored_number = 0.0;
        HistoryItem {
            average: ignored_number,
            date: date.to_string(),
            highest: ignored_number,
            lowest: ignored_number,
            order_count: ignored_number as i64,
            volume,
        }
    }

    #[test]
    fn test_lowest_station_sell_price() {
        let region_orders = RegionOrders {
            region: Region::new(1, "Region", vec![]),
            orders: vec![
                create_market_order(100, 1, 1000, OrderType::Sell, 100.0),
                create_market_order(100, 1, 1000, OrderType::Buy, 50.0),
                create_market_order(100, 1, 1000, OrderType::Sell, 75.0),
                create_market_order(100, 1, 1001, OrderType::Sell, 60.0),
                create_market_order(100, 2, 1000, OrderType::Sell, 25.0),
            ],
        };

        let res = region_orders.lowest_station_sell_price(1, 1000);
        assert_eq!(res, 75.0)
    }

    #[tokio::test]
    async fn test_lowest_sell_price() {
        let requester = Arc::new(
            MockRequester::builder()
                .insert_region_order(3, create_market_order(10, 100, 8, OrderType::Sell, 100.0))
                .insert_region_order(3, create_market_order(10, 100, 8, OrderType::Sell, 75.0))
                .insert_region_order(3, create_market_order(11, 100, 8, OrderType::Buy, 25.0))
                .insert_region_order(3, create_market_order(12, 100, 7, OrderType::Sell, 50.0))
                .insert_region_order(3, create_market_order(13, 108, 8, OrderType::Sell, 45.0))
                .build(),
        );
        let facility = create_facility(requester);
        let res = facility
            .market()
            .unwrap()
            .lowest_sell_price(100, OrdersRange::Station)
            .await
            .unwrap();
        assert_eq!(res, Some(75.0))
    }

    #[tokio::test]
    async fn test_regional_average_volume() {
        let requester = Arc::new(
            MockRequester::builder()
                .insert_history_item(3, 100, create_history_item("2024-01-31", 25))
                .insert_history_item(3, 100, create_history_item("2024-01-30", 75))
                .insert_history_item(3, 100, create_history_item("2024-01-29", 8))
                .insert_history_item(3, 100, create_history_item("2024-01-28", 10))
                .insert_history_item(3, 100, create_history_item("2024-01-27", 20))
                .build(),
        );
        let facility = create_facility(requester);
        let period = NaivePeriod::new(
            NaiveDate::from_ymd_opt(2024, 01, 28).unwrap(),
            NaiveDate::from_ymd_opt(2024, 01, 31).unwrap(),
        );
        let res = facility
            .market()
            .unwrap()
            .regional_average_volume(100, period)
            .await
            .unwrap();
        assert_eq!(res, 29)
    }
}
