use crate::commands::manufacturing::all::manufacture_all;
use crate::commands::manufacturing::item::manufacture;
use crate::configuration::cli::{ManufacturingCommands, ManufacturingOptions};
use crate::errors::EveError;
use crate::integration::DataIntegrator;

mod item;

mod all;

pub async fn manufacturing(
    eve: &DataIntegrator,
    opts: &ManufacturingOptions,
) -> Result<(), EveError> {
    match &opts.command {
        ManufacturingCommands::Item(item_opts) => manufacture(eve, opts, item_opts).await?,
        ManufacturingCommands::All(all_opts) => {
            manufacture_all(eve, opts, all_opts).await?;
        }
    }
    Ok(())
}
