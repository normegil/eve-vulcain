use inquire::Select;
use std::fmt::{Display, Formatter};

use crate::errors::{EnvironmentError, EveError, ModelError};
use crate::integration::DataIntegrator;
use crate::interactive::HandleInquireExitSignals;
use crate::model;
use crate::model::common::{Identified, Named};

pub async fn rm(eve: &DataIntegrator) -> Result<(), EveError> {
    let items = eve.load_registered_items().await?;
    let items = Item::from(items);

    let item = Select::new("Please choose an item to remove: ", items)
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "select item".to_string(),
            source,
        })?;
    let item = match item {
        Some(item) => item,
        None => return Ok(()),
    };

    eve.fs()
        .rm_item(item.id)
        .await
        .map_err(|source| ModelError::RemovingItem { source })?;
    Ok(())
}

pub struct Item {
    id: i32,
    name: String,
}

impl Item {
    pub fn from(items: Vec<model::items::Item>) -> Vec<Self> {
        items
            .iter()
            .map(|i| Self {
                id: i.id(),
                name: i.name(),
            })
            .collect()
    }
}

impl Display for Item {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
