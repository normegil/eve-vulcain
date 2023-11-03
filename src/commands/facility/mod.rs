use crate::commands::facility::add::add;
use crate::commands::facility::ls::ls;
use crate::commands::facility::rm::rm;
use crate::configuration::cli::{FacilityCommands, FacilityOptions};
use crate::errors::EveError;
use crate::integration::DataIntegrator;

pub mod add;
mod ls;
mod rm;

pub async fn facility(eve: &DataIntegrator, opts: &FacilityOptions) -> Result<(), EveError> {
    match &opts.command {
        FacilityCommands::Add(_) => {
            add(eve).await?;
        }
        FacilityCommands::Rm => {
            rm(eve).await?;
        }
        FacilityCommands::Ls => {
            ls(eve).await?;
        }
    }
    Ok(())
}
