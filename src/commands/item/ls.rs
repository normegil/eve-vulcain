use colored::ColoredString;
use serde::Serialize;

use crate::{
    errors::EveError,
    integration::DataIntegrator,
    logging::{self, Message, Stdout, Verbosity},
    model::{common::Named, items::Item},
};

pub async fn ls(eve: &DataIntegrator) -> Result<(), EveError> {
    let items = eve.load_registered_items().await?;
    logging::stdoutln(ItemsLSStdout::from(items))?;
    Ok(())
}

#[derive(Serialize)]
struct ItemsLSStdout {
    items: Vec<ItemStdout>,
}

impl Stdout for ItemsLSStdout {}

impl ItemsLSStdout {
    pub fn from(items: Vec<Item>) -> Self {
        let items = items.iter().map(ItemStdout::from).collect();
        Self { items }
    }
}

impl Message for ItemsLSStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let mut all_items = String::new();
        for item in &self.items {
            all_items += item.standard(verbosity).to_string().as_str();
        }
        ColoredString::from(all_items.to_string().as_str())
    }
}

#[derive(Serialize)]
pub struct ItemStdout {
    name: String,
}

impl ItemStdout {
    pub fn from(item: &Item) -> Self {
        Self { name: item.name() }
    }
}

impl Message for ItemStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        ColoredString::from(format!("{}\n", self.name).as_str())
    }
}
