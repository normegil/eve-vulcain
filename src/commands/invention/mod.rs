use crate::{
    configuration::cli::{InventionCommands, InventionOptions},
    errors::EveError,
    integration::DataIntegrator,
};

mod item;

pub async fn invention(eve: &DataIntegrator, opts: &InventionOptions) -> Result<(), EveError> {
    match &opts.command {
        InventionCommands::Item(item_opts) => item::invention(eve, item_opts).await?,
    }
    Ok(())
}
