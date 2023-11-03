use crate::model::common::{DetailedCalculation, Identified, Named};
use crate::model::prices::Prices;

use super::skills::Skill;

#[derive(Debug, Clone, PartialEq)]
pub struct Blueprint {
    pub id: i32,
    pub activities: Activities,
}

impl Identified<i32> for Blueprint {
    fn id(&self) -> i32 {
        self.id
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Activities {
    pub manufacturing: Option<BlueprintManufacturing>,
    pub invention: Option<BlueprintInvention>,
}

pub struct ManufacturingEnvironment {
    pub material_efficiency: u8,
    pub material_consumption_modifier: Option<f64>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct BlueprintInvention {
    pub blueprint_id: i32,
    pub materials: Materials,
    pub products: Vec<ProbableMultipleItems>,
    pub skills: Vec<Skill>,
    pub time: i32,
}

impl BlueprintInvention {
    pub fn get_product(&self, product_id: i32) -> Option<&ProbableMultipleItems> {
        self.products
            .iter()
            .find(|&prod| product_id == prod.item.id())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ProbableMultipleItems {
    pub quantity: i32,
    pub base_probability: Option<f64>,
    pub item: crate::model::items::Item,
}

#[derive(Debug, PartialEq, Clone)]
pub struct BlueprintManufacturing {
    pub blueprint_id: i32,
    pub materials: Materials,
    pub products: Vec<MultipleItems>,
    pub material_efficiency: u8,
    pub time_efficiency: u8,
    pub time: i32,
    pub invention_blueprint: Vec<BlueprintInvention>,
}

impl Identified<i32> for BlueprintManufacturing {
    fn id(&self) -> i32 {
        self.blueprint_id
    }
}

impl BlueprintManufacturing {
    pub fn get_product(&self, product_id: i32) -> Option<&MultipleItems> {
        self.products
            .iter()
            .find(|&prod| product_id == prod.item.id())
    }

    pub fn estimated_item_value(&self, prices: &Prices) -> f64 {
        let mut val = 0.0;
        for material in &self.materials.0 {
            let item_prices = prices.get(material.item.id());
            if let Some(item_prices) = item_prices {
                val += (material.quantity as f64) * item_prices.get_adjusted().unwrap_or(0.0);
            }
        }
        val
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct MultipleItems {
    pub quantity: i32,
    pub item: crate::model::items::Item,
}

impl MultipleItems {
    pub fn effective_quantity(&self, env: &ManufacturingEnvironment) -> i32 {
        let qt = self.quantity;

        let material_efficiency_normalized = (env.material_efficiency as f64) / 100.0;
        let material_efficiency_reduction = (qt as f64) * material_efficiency_normalized;

        let material_consumption_modifier_reduction = match env.material_consumption_modifier {
            None => 0,
            Some(modifier) => (qt as f64 * modifier).round() as i32,
        };
        qt - (material_efficiency_reduction as i32) - material_consumption_modifier_reduction
    }

    pub fn cost(
        &self,
        env: &ManufacturingEnvironment,
        average_price: f64,
    ) -> DetailedCalculation<f64, MultipleItemsCostDetails> {
        let effective_quantity = self.effective_quantity(env);
        let total = (effective_quantity as f64) * average_price;
        DetailedCalculation {
            value: total,
            details: MultipleItemsCostDetails {
                id: self.item.id(),
                name: self.item.name(),
                orders: vec![MultipleItemsOrdersDetails {
                    effective_quantity,
                    price_per_unit: average_price,
                    total,
                }],
                total,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct InputMaterialsCostsDetails {
    pub costs: Vec<MultipleItemsCostDetails>,
}

#[derive(Debug, Clone)]
pub struct MultipleItemsCostDetails {
    pub id: i32,
    pub name: String,
    pub orders: Vec<MultipleItemsOrdersDetails>,
    pub total: f64,
}

#[derive(Debug, Clone)]
pub struct MultipleItemsOrdersDetails {
    pub effective_quantity: i32,
    pub price_per_unit: f64,
    pub total: f64,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Materials(Vec<MultipleItems>);

impl Materials {
    pub fn new(items: Vec<MultipleItems>) -> Self {
        Self(items)
    }

    pub fn input_materials_cost(
        &self,
        material_consumption_modifier: Option<f64>,
        material_efficiency: Option<u8>,
        prices: &Prices,
    ) -> DetailedCalculation<f64, InputMaterialsCostsDetails> {
        let manufacturing_environment = ManufacturingEnvironment {
            material_efficiency: material_efficiency.unwrap_or(0),
            material_consumption_modifier,
        };

        let mut total = 0.0;
        let mut orders = vec![];
        for material in &self.0 {
            let item_prices = prices.get(material.item.id());
            if let Some(item_prices) = item_prices {
                let average_price = item_prices.get_average().unwrap_or(0.0);
                let material_cost = material.cost(&manufacturing_environment, average_price);
                orders.push(material_cost.details.clone());
                total += material_cost.value;
            }
        }
        DetailedCalculation {
            value: total,
            details: InputMaterialsCostsDetails { costs: orders },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::model::{
        items::{Item, TechLevel},
        prices::ItemPrice,
    };

    use super::*;

    #[test]
    fn test_effective_quantity_no_modifier() {
        let env = ManufacturingEnvironment {
            material_efficiency: 50,
            material_consumption_modifier: None,
        };

        let item = MultipleItems {
            quantity: 10,
            item: Item::new(1, "Test", None, TechLevel::One),
        };

        assert_eq!(item.effective_quantity(&env), 5); // 10 - (10 * 0.5)
    }

    #[test]
    fn test_effective_quantity_with_modifier() {
        let env = ManufacturingEnvironment {
            material_efficiency: 25,
            material_consumption_modifier: Some(0.2),
        };

        let item = MultipleItems {
            quantity: 8,
            item: Item::new(1, "Test", None, TechLevel::One),
        };

        assert_eq!(item.effective_quantity(&env), 4); // 8 - (8 * 0.25) - (8 * 0.2).round()
    }

    #[test]
    fn test_cost_no_modifier() {
        let env = ManufacturingEnvironment {
            material_efficiency: 50,
            material_consumption_modifier: None,
        };

        let item = MultipleItems {
            quantity: 10,
            item: Item::new(1, "Test", None, TechLevel::One),
        };

        let average_price = 2.5;
        let result = item.cost(&env, average_price);

        assert_eq!(result.value, 12.5); // 10 * 2.5
        assert_eq!(result.details.id, 1);
        assert_eq!(result.details.name, "Test");
        assert_eq!(result.details.total, 12.5);
        assert_eq!(result.details.orders.len(), 1);
        assert_eq!(result.details.orders[0].effective_quantity, 5); // 10 - (10 * 0.5)
        assert_eq!(result.details.orders[0].price_per_unit, average_price);
        assert_eq!(result.details.orders[0].total, 12.5);
    }

    #[test]
    fn test_cost_with_modifier() {
        let env = ManufacturingEnvironment {
            material_efficiency: 25,
            material_consumption_modifier: Some(0.2),
        };

        let item = MultipleItems {
            quantity: 8,
            item: Item::new(1, "Test", None, TechLevel::One),
        };

        let average_price = 3.0;
        let result = item.cost(&env, average_price);

        assert_eq!(result.details.id, 1);
        assert_eq!(result.details.name, "Test");
        assert_eq!(result.details.orders.len(), 1);
        assert_eq!(result.details.orders[0].effective_quantity, 4); // 8 - (8 * 0.25) - (8 * 0.2).round()
        assert_eq!(result.details.orders[0].price_per_unit, average_price);
        assert_eq!(result.details.orders[0].total, 12.0);
        assert_eq!(result.details.total, 12.0);
        assert_eq!(result.value, 12.0);
    }

    #[test]
    fn test_input_materials_cost_no_modifier() {
        let items = vec![
            MultipleItems {
                quantity: 5,
                item: Item::new(1, "Test", None, TechLevel::One),
            },
            MultipleItems {
                quantity: 10,
                item: Item::new(2, "Test", None, TechLevel::One),
            },
        ];

        let mut prices = HashMap::new();
        prices.insert(1, ItemPrice::new(None, Some(2.5)));
        prices.insert(2, ItemPrice::new(None, Some(3.5)));
        let prices = Prices { prices };

        let materials = Materials::new(items);

        let result = materials.input_materials_cost(None, Some(50), &prices);

        assert_eq!(result.value, 25.0);
        assert_eq!(result.details.costs.len(), 2);
        assert_eq!(result.details.costs[0].total, 7.5);
        assert_eq!(result.details.costs[1].total, 17.5);
    }

    #[test]
    fn test_input_materials_cost_with_modifier() {
        let items = vec![
            MultipleItems {
                quantity: 5,
                item: Item::new(1, "Test", None, TechLevel::One),
            },
            MultipleItems {
                quantity: 10,
                item: Item::new(2, "Test", None, TechLevel::One),
            },
        ];

        let mut prices = HashMap::new();
        prices.insert(1, ItemPrice::new(None, Some(2.5)));
        prices.insert(2, ItemPrice::new(None, Some(3.5)));
        let prices = Prices { prices };

        let materials = Materials::new(items);

        let result = materials.input_materials_cost(Some(0.2), Some(25), &prices);

        assert_eq!(result.value, 28.5);
        assert_eq!(result.details.costs.len(), 2);
        assert_eq!(result.details.costs[0].total, 7.5);
        assert_eq!(result.details.costs[1].total, 21.0);
    }

    #[test]
    fn test_estimated_item_value() {
        let items = vec![
            MultipleItems {
                quantity: 5,
                item: Item::new(1, "Test", None, TechLevel::One),
            },
            MultipleItems {
                quantity: 10,
                item: Item::new(2, "Test", None, TechLevel::One),
            },
        ];

        let materials = Materials::new(items);

        let blueprint = BlueprintManufacturing {
            blueprint_id: 1,
            materials,
            products: vec![],
            material_efficiency: 25,
            time_efficiency: 10,
            time: 120,
            invention_blueprint: Vec::new(),
        };

        let mut prices = HashMap::new();
        prices.insert(1, ItemPrice::new(Some(2.0), None));
        prices.insert(2, ItemPrice::new(Some(3.5), None));
        let prices = Prices { prices };

        let result = blueprint.estimated_item_value(&prices);

        assert_eq!(result, 45.0);
    }
}
