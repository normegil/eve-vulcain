use std::fmt::Formatter;

use serde::{
    de::{self, Unexpected, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use strum_macros::{Display, EnumString};

use std::str::FromStr;

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub struct SearchKey {
    pub character_id: i32,
    pub categories: String,
    pub search: String,
    pub strict: Option<bool>,
}

impl Serialize for SearchKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let strict_str = match self.strict {
            None => "None".to_string(),
            Some(val) => {
                format!("{}", val)
            }
        };
        serializer.serialize_str(
            format!(
                "{}///{}///{}///{}",
                self.character_id, self.categories, self.search, strict_str
            )
            .as_str(),
        )
    }
}

impl<'de> Deserialize<'de> for SearchKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(SearchKeyVisitor)
    }
}

struct SearchKeyVisitor;

impl<'de> Visitor<'de> for SearchKeyVisitor {
    type Value = SearchKey;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("search endpoint arguments from 4 values separated by '///'.")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let mut split = s.split("///");
        let character_id = split
            .next()
            .ok_or(de::Error::missing_field("character_id"))?;
        let categories = split
            .next()
            .ok_or(de::Error::missing_field("character_id"))?;
        let search = split.next().ok_or(de::Error::missing_field("search"))?;
        let strict = split.next().ok_or(de::Error::missing_field("strict"))?;
        if split.next().is_some() {
            return Err(de::Error::custom(
                "Invalid length - Unrecognized extra option",
            ));
        }

        let character_id = character_id.parse::<i32>().map_err(|source| {
            de::Error::custom(format!("Invalid character_id '{character_id}': {source}").as_str())
        })?;
        let strict = if strict == "None" {
            None
        } else if strict == "true" {
            Some(true)
        } else if strict == "false" {
            Some(false)
        } else {
            return Err(de::Error::invalid_value(Unexpected::Str(strict), &self));
        };

        Ok(SearchKey {
            character_id,
            categories: categories.to_string(),
            search: search.to_string(),
            strict,
        })
    }
}

#[derive(Eq, PartialEq, Hash, Debug, Clone, Serialize, Deserialize, EnumString, Display)]
pub enum OrderType {
    Buy,
    Sell,
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub struct MarketOrderKey {
    pub region_id: i32,
    pub order_type: OrderType,
}

impl Serialize for MarketOrderKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(format!("{}///{}", self.region_id, self.order_type).as_str())
    }
}

impl<'de> Deserialize<'de> for MarketOrderKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(MarketOrderKeyVisitor)
    }
}
struct MarketOrderKeyVisitor;

impl<'de> Visitor<'de> for MarketOrderKeyVisitor {
    type Value = MarketOrderKey;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("market order endpoint arguments from 2 values separated by '///'.")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let mut split = s.split("///");
        let region_id = split.next().ok_or(de::Error::missing_field("region_id"))?;
        let order_type = split.next().ok_or(de::Error::missing_field("order_type"))?;

        let region_id = region_id.parse::<i32>().map_err(|source| {
            de::Error::custom(format!("Invalid region_id '{region_id}': {source}").as_str())
        })?;
        let order_type = OrderType::from_str(order_type).map_err(|source| {
            de::Error::custom(format!("Invalid order_type '{order_type}': {source}").as_str())
        })?;

        Ok(MarketOrderKey {
            region_id,
            order_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_search_key_serialization() {
        let search_key = SearchKey {
            character_id: 123,
            categories: String::from("test_category"),
            search: String::from("test_search"),
            strict: Some(true),
        };

        let serialized = serde_json::to_string(&search_key).unwrap();
        let expected = r#""123///test_category///test_search///true""#;
        assert_eq!(serialized, expected);

        let deserialized: SearchKey = serde_json::from_str(expected).unwrap();
        assert_eq!(deserialized, search_key);
    }

    #[test]
    fn test_market_order_key_serialization() {
        let market_order_key = MarketOrderKey {
            region_id: 456,
            order_type: OrderType::Sell,
        };

        let serialized = serde_json::to_string(&market_order_key).unwrap();
        let expected = r#""456///Sell""#;
        assert_eq!(serialized, expected);

        let deserialized: MarketOrderKey = serde_json::from_str(expected).unwrap();
        assert_eq!(deserialized, market_order_key);
    }
}
