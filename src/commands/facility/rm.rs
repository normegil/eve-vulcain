use std::fmt::{Display, Formatter};

use inquire::Select;

use crate::errors::{EnvironmentError, EveError, ModelError};
use crate::integration::DataIntegrator;
use crate::interactive::HandleInquireExitSignals;
use crate::model::common::{Identified, Named};
use crate::model::facility::{Facility, FacilityType};

pub async fn rm(eve: &DataIntegrator) -> Result<(), EveError> {
    let facilities = eve.load_registered_facilities().await?;
    let facilities = facilities.into_iter().map(FacilityDisplay).collect();

    let facility = Select::new("Please choose a facility to remove: ", facilities)
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "select facility to remove".to_string(),
            source,
        })?;
    let facility = match facility {
        Some(facility) => facility,
        None => return Ok(()),
    };

    match facility.0.facility_type() {
        FacilityType::Station(s) => {
            eve.fs()
                .rm_station(s.id())
                .await
                .map_err(|source| ModelError::RemovingNPCStation { source })?;
        }
        FacilityType::Structure(s) => {
            eve.fs()
                .rm_structure(s.id())
                .await
                .map_err(|source| ModelError::RemovingPlayerStructure { source })?;
        }
    }

    Ok(())
}

pub struct FacilityDisplay(Facility);

impl Display for FacilityDisplay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.name())
    }
}
