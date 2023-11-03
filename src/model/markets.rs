use crate::api::evecache::cache_keys::OrderType;

use super::items::Item;

#[derive(Debug, PartialEq)]
pub struct CharacterOrder {
    pub item: Item,
    pub order_type: OrderType,
    pub price: f64,
    pub volume_remain: i32,
    pub volume_total: i32,
}
