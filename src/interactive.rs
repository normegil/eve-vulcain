use futures_util::future::TryJoinAll;
use inquire::{InquireError, Select};
use tokio::try_join;

use crate::{
    errors::{EnvironmentError, EveError, ModelError},
    integration::{DataIntegrator, LoadFrom},
    logging,
    model::{
        blueprint::Blueprint,
        common::Identified,
        industry::IndustryType,
        items::{Item, TechLevel},
    },
};

pub trait HandleInquireExitSignals<T> {
    fn handle_exit_signals(self) -> Result<Option<T>, InquireError>;
}

impl<T> HandleInquireExitSignals<T> for Result<T, InquireError> {
    fn handle_exit_signals(self) -> Result<Option<T>, InquireError> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(InquireError::OperationCanceled) => Ok(None),
            Err(InquireError::OperationInterrupted) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

pub async fn load_item(
    eve: &DataIntegrator,
    item_name: String,
    strict: bool,
    tech_level: Option<TechLevel>,
) -> Result<Option<Item>, EveError> {
    logging::debug!("Item name: {:?}", item_name);
    let items = eve
        .search_items(LoadFrom::Name(item_name.clone()), strict)
        .await?;
    logging::debug!("Search item: {:?}", item_name);

    let mut futures = vec![];
    for item in items {
        futures.push(async {
            let bps = eve
                .load_item_blueprints(item.id(), IndustryType::Manufacturing)
                .await?;
            Ok::<(Item, Vec<Blueprint>), EveError>((item, bps))
        })
    }
    let all_futures = futures.into_iter().collect::<TryJoinAll<_>>();
    let item_bps_associations = try_join!(all_futures)?.0;
    logging::debug!("Blueprints loaded: {:?}", item_name);

    let mut filtered_items: Vec<Item> = item_bps_associations
        .into_iter()
        .filter(|(_, bps)| {
            for bp in bps {
                if bp.activities.manufacturing.is_some() {
                    return true;
                }
            }
            false
        })
        .map(|(i, _)| i)
        .collect();
    if let Some(tech_level) = tech_level {
        filtered_items.retain(|i| i.tech_level == tech_level);
    }
    logging::debug!("Filtered");

    let searched_item = match filtered_items.len() {
        x if x < 1 => {
            return Err(ModelError::NoItemFound {
                name: item_name.clone(),
            })?;
        }
        1 => filtered_items.remove(0),
        _ => {
            let result_searched_item = Select::new(
                "Please select the item you're looking for: ",
                filtered_items,
            )
            .prompt()
            .handle_exit_signals()
            .map_err(|source| EnvironmentError::SpecificInputError {
                description: "item to manufacture".to_string(),
                source,
            })?
            .clone();
            match result_searched_item {
                Some(searched_item) => searched_item,
                None => return Ok(None),
            }
        }
    };
    Ok(Some(searched_item))
}
