use crate::commands::item::add::add;
use crate::commands::item::ls::ls;
use crate::commands::item::rm::rm;
use crate::configuration::cli::{ItemCommands, ItemOptions};
use crate::errors::EveError;
use crate::integration::DataIntegrator;

pub mod add;
mod ls;
mod rm;

pub async fn item(eve: &DataIntegrator, opts: &ItemOptions) -> Result<(), EveError> {
    match &opts.command {
        ItemCommands::Add(add_opts) => {
            add(eve, add_opts).await?;
        }
        ItemCommands::Rm => {
            rm(eve).await?;
        }
        ItemCommands::Ls => {
            ls(eve).await?;
        }
    }
    Ok(())
}
