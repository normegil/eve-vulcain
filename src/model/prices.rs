use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Prices {
    pub prices: HashMap<i32, ItemPrice>,
}

impl Prices {
    pub fn new(prices: HashMap<i32, ItemPrice>) -> Self {
        Self { prices }
    }

    pub fn get(&self, item_id: i32) -> Option<&ItemPrice> {
        self.prices.get(&item_id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemPrice {
    adjusted: Option<f64>,
    average: Option<f64>,
}

impl ItemPrice {
    pub fn new(adjusted: Option<f64>, average: Option<f64>) -> Self {
        Self { adjusted, average }
    }

    pub fn get_adjusted(&self) -> Option<f64> {
        self.adjusted
    }

    pub fn get_average(&self) -> Option<f64> {
        self.average
    }
}

#[derive(Debug)]
pub enum PriceType {
    // Adjusted,
    // Average,
}
