use std::collections::HashMap;

use inquire::{min_length, required, Confirm, Select, Text};

use crate::api::sde::{BlueprintActivityType, SDEBlueprint};
use crate::configuration::cli::ItemAddOptions;
use crate::errors::{EnvironmentError, EveError, ModelError};
use crate::integration::{DataIntegrator, LoadFrom};
use crate::interactive::HandleInquireExitSignals;
use crate::logging;
use crate::logging::Msg;
use crate::model::common::Identified;

pub async fn add(eve: &DataIntegrator, opts: &ItemAddOptions) -> Result<(), EveError> {
    let blueprints = eve
        .api()
        .get_blueprints()
        .await
        .map_err(|source| ModelError::LoadBlueprints { source })?;
    logging::println(Msg(
        "You're about to add an item used by other eve-vulcain commands.".to_string(),
    ));

    loop {
        if add_item(eve, opts, &blueprints).await? {
            return Ok(());
        }

        logging::println(Msg("".to_string()));
        let res = Confirm::new("Do you want to add another item ?")
            .with_default(true)
            .prompt()
            .handle_exit_signals()
            .map_err(
                |source: inquire::InquireError| EnvironmentError::SpecificInputError {
                    description: "item continue".to_string(),
                    source,
                },
            )?;
        if let Some(false) | None = res {
            return Ok(());
        }
        logging::println(Msg("".to_string()));
    }
}

async fn add_item(
    eve: &DataIntegrator,
    opts: &ItemAddOptions,
    blueprints: &HashMap<i32, SDEBlueprint>,
) -> Result<bool, EveError> {
    let mut default_stop = false;
    let search = match &opts.item {
        None => {
            let search = Text::new("Search an item by name: ")
                .with_help_message("3 characters minimum.")
                .with_validator(required!())
                .with_validator(min_length!(3, "3 characters required."))
                .prompt()
                .handle_exit_signals()
                .map_err(|source| EnvironmentError::SpecificInputError {
                    description: "item_name".to_string(),
                    source,
                })?;
            let search = match search {
                Some(search) => search,
                None => return Ok(true),
            };
            search.trim().to_string()
        }
        Some(s) => {
            default_stop = true;
            s.clone()
        }
    };

    logging::trace!("Search for item name: '{}'", search);
    let items = eve
        .search_items(LoadFrom::Name(search.to_string()), false)
        .await?;
    if items.is_empty() {
        return Err(ModelError::SearchedItemNotFound {
            search: search.to_string(),
        })?;
    }

    let items = items
        .into_iter()
        .filter(|i| can_manufacture(blueprints, i.id()))
        .collect();

    let item = Select::new("Select one of the found items ", items)
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "select item".to_string(),
            source,
        })?;
    let item = match item {
        Some(item) => item,
        None => return Ok(true),
    };
    eve.fs()
        .add_item(item.id())
        .await
        .map_err(|source| ModelError::SaveItemError { source })?;

    Ok(default_stop)
}

fn can_manufacture(blueprints: &HashMap<i32, SDEBlueprint>, item_id: i32) -> bool {
    for (_, blueprint) in blueprints.iter() {
        if blueprint.can_produce(item_id, &BlueprintActivityType::Manufacturing) {
            return true;
        }
    }
    false
}
