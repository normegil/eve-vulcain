use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use futures_util::future::TryJoinAll;
use inquire::validator::{ErrorMessage, StringValidator, Validation};
use inquire::{min_length, required, Confirm, CustomUserError, Select, Text};
use strum::IntoEnumIterator;
use tokio::try_join;

use crate::errors::{EnvironmentError, EveError, ModelError};
use crate::filesystem::{NPCStation, PlayerStructure};
use crate::integration::{DataIntegrator, DataLoadError, FacilityLoadingError, SystemLoadingError};
use crate::interactive::HandleInquireExitSignals;
use crate::logging;
use crate::logging::Msg;
use crate::model::common::{Identified, Named};
use crate::model::facility::playerstructure::PlayerStructureStats;
use crate::model::facility::{Facility, FacilityType, FacilityUsage};
use crate::model::industry::IndustryType;
use crate::model::locations::{Constellation, SolarSystem};

pub async fn add(eve: &DataIntegrator) -> Result<(), EveError> {
    logging::println(Msg(
        "You're about to add facilities used by other eve-vulcain commands.".to_string(),
    ));

    let option: Vec<FacilityType> = FacilityType::iter().collect();
    logging::info!("Facility options loaded");

    loop {
        if add_facility(eve, option.clone()).await? {
            return Ok(());
        }

        logging::println(Msg("".to_string()));
        let res = Confirm::new("Do you want to add another facility ?")
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

async fn add_facility(eve: &DataIntegrator, option: Vec<FacilityType>) -> Result<bool, EveError> {
    let facility_type = Select::new("Please choose a facility type: ", option)
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "facility type".to_string(),
            source,
        })?
        .clone();
    let facility_type = match facility_type {
        Some(facility_type) => facility_type,
        None => return Ok(true),
    };

    logging::info!("Selected facility type");
    match facility_type {
        FacilityType::Station(_) => add_npc_station(eve).await,
        FacilityType::Structure(_) => add_player_structure(eve).await,
    }
}

async fn add_npc_station(eve: &DataIntegrator) -> Result<bool, EveError> {
    let regions = eve.load_all_regions().await?;
    let regions = regions
        .into_iter()
        .map(|region| Region { region })
        .collect();

    let region = Select::new("Please choose a region: ", regions)
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "select region".to_string(),
            source,
        })?;
    let region = match region {
        Some(region) => region,
        None => return Ok(true),
    };
    logging::trace!("You've chosen {}", region.region.name());

    let systems = load_systems(eve, &region).await?;
    let systems = systems
        .into_iter()
        .map(|system| System { system })
        .collect();

    let system = Select::new("Please choose a system: ", systems)
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "select system".to_string(),
            source,
        })?;
    let system = match system {
        Some(system) => system,
        None => return Ok(true),
    };
    logging::trace!("You've chosen {}", system.system.name());

    if system.system.station_ids.is_empty() {
        return Err(ModelError::NoStationFoundInSystem {
            system_name: system.system.name(),
            region_name: region.region.name(),
        })?;
    }

    let mut futures = Vec::new();
    for station_id in &system.system.station_ids {
        futures.push(async move {
            let sta = eve.load_station(*station_id).await?;
            Ok::<Facility, FacilityLoadingError>(sta)
        })
    }
    let all_operations = futures.into_iter().collect::<TryJoinAll<_>>();
    let stations = try_join!(all_operations)
        .map_err(|source| ModelError::LoadStationInSystemError {
            system_id: system.system.id(),
            system_name: system.system.name(),
            source,
        })?
        .0;

    let stations = stations
        .into_iter()
        .map(|station| Station { station })
        .collect();
    let station = Select::new("Please choose a station: ", stations)
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "select station".to_string(),
            source,
        })?;
    let station = match station {
        Some(station) => station,
        None => return Ok(true),
    };
    logging::trace!("You've chosen {}", station.station.name());

    let facility_usage = Select::new(
        "Which type of usage will this facility be registered for ",
        vec!["Industry", "Market", "Both"],
    )
    .prompt()
    .handle_exit_signals()
    .map_err(|source| EnvironmentError::SpecificInputError {
        description: "facility usage".to_string(),
        source,
    })?;
    let facility_usage = match facility_usage {
        Some(facility_usage) => facility_usage,
        None => return Ok(true),
    };

    logging::trace!("You've chosen {}", facility_usage);

    let mut usages = vec![];
    if "Industry" == facility_usage || "Both" == facility_usage {
        usages.push(FacilityUsage::Industry);
    }
    if "Market" == facility_usage || "Both" == facility_usage {
        usages.push(FacilityUsage::Market);
    }
    let station = NPCStation {
        id: station.station.id() as i32,
        usages,
    };
    eve.fs()
        .add_station(&station)
        .await
        .map_err(|source| ModelError::SaveStationError { source })?;

    Ok(false)
}

async fn add_player_structure(eve: &DataIntegrator) -> Result<bool, EveError> {
    let search = Text::new("Search a structure name: ")
        .with_help_message("3 characters minimum.")
        .with_validator(required!())
        .with_validator(min_length!(3, "3 characters required."))
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "search structure by name".to_string(),
            source,
        })?;
    let search = match search {
        Some(search) => search,
        None => return Ok(true),
    };
    let search = search.trim();

    logging::trace!("Search for structure name: '{}'", search);

    let structures = eve.search_structure(search).await?;
    if structures.is_empty() {
        return Err(ModelError::SearchedStructureNotFound {
            search: search.to_string(),
        })?;
    }
    let structures = structures
        .into_iter()
        .map(|s| Structure { structure: s })
        .collect();

    let structure = Select::new("Select one of the found structures ", structures)
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "select structure".to_string(),
            source,
        })?;
    let structure = match structure {
        Some(structure) => structure,
        None => return Ok(true),
    };
    logging::trace!(
        "Search for structure name: '{}'",
        structure.structure.name()
    );

    let facility_usage = Select::new(
        "Which type of usage will this facility be registered for ",
        vec!["Industry", "Market", "Both"],
    )
    .prompt()
    .handle_exit_signals()
    .map_err(|source| EnvironmentError::SpecificInputError {
        description: "select facility usage".to_string(),
        source,
    })?;
    let facility_usage = match facility_usage {
        Some(facility_usage) => facility_usage,
        None => return Ok(true),
    };
    logging::trace!("You've chosen {}", facility_usage);

    let mut facility_usages = vec![];
    let mut activities: HashMap<IndustryType, PlayerStructureStats> = Default::default();
    if "Market" == facility_usage || "Both" == facility_usage {
        facility_usages.push(FacilityUsage::Market);
    }
    if "Industry" == facility_usage || "Both" == facility_usage {
        facility_usages.push(FacilityUsage::Industry);
        let mut industry_types: Vec<String> = IndustryType::iter().map(|t| t.to_string()).collect();
        industry_types.push("None".to_string());

        loop {
            logging::println(Msg(String::from("")));
            let industry_type = Select::new("Configure an industry type ", industry_types.clone())
                .with_help_message("Choose 'None' to stop configuring industries.")
                .prompt()
                .handle_exit_signals()
                .map_err(|source| EnvironmentError::SpecificInputError {
                    description: "select industry type".to_string(),
                    source,
                })?;
            let industry_type = match industry_type {
                Some(industry_type) => industry_type,
                None => return Ok(true),
            };
            logging::trace!("You've chosen {}", industry_type);

            if "None" == industry_type {
                break;
            }

            let index = industry_types.iter().position(|t| t == &industry_type);
            if let Some(i) = index {
                industry_types.remove(i);
            }

            let tax_rate = Text::new(
                format!(
                    "What's the structure tax rate for {} (in %): ",
                    industry_type
                )
                .as_str(),
            )
            .with_validator(required!())
            .with_validator(RateValidator {})
            .with_help_message("From 0 to 99%.")
            .prompt()
            .handle_exit_signals()
            .map_err(|source| EnvironmentError::SpecificInputError {
                description: "structure tax rate".to_string(),
                source,
            })?;
            let tax_rate = match tax_rate {
                Some(tax_rate) => tax_rate,
                None => return Ok(true),
            };
            let tax_rate = tax_rate.parse::<f64>().unwrap() / 100.0;

            let job_cost_modifier = Text::new(
                format!(
                    "What's the total job cost modifier for {} (in %): ",
                    industry_type
                )
                .as_str(),
            )
            .with_validator(RateValidator {})
            .with_help_message("From 0 to 99%. Push Enter if the modifier doesn't exist.")
            .prompt()
            .handle_exit_signals()
            .map_err(|source| EnvironmentError::SpecificInputError {
                description: "job cost modifier".to_string(),
                source,
            })?;
            let job_cost_modifier = match job_cost_modifier {
                Some(job_cost_modifier) => job_cost_modifier,
                None => return Ok(true),
            };
            let job_cost_modifier = if job_cost_modifier.is_empty() {
                None
            } else {
                Some(job_cost_modifier.parse::<f64>().unwrap() / 100.0)
            };

            let material_consumption_modifier = Text::new(
                format!(
                    "What's the total material consumption modifier for {} (in %): ",
                    industry_type
                )
                .as_str(),
            )
            .with_validator(RateValidator {})
            .with_help_message("From 0 to 99%. Push Enter if the modifier doesn't exist.")
            .prompt()
            .handle_exit_signals()
            .map_err(|source| EnvironmentError::SpecificInputError {
                description: "material consumption modifier".to_string(),
                source,
            })?;
            let material_consumption_modifier = match material_consumption_modifier {
                Some(material_consumption_modifier) => material_consumption_modifier,
                None => return Ok(true),
            };
            let material_consumption_modifier = if material_consumption_modifier.is_empty() {
                None
            } else {
                Some(material_consumption_modifier.parse::<f64>().unwrap() / 100.0)
            };

            let job_duration_modifier = Text::new(
                format!(
                    "What's the total job duration modifier for {} (in %): ",
                    industry_type
                )
                .as_str(),
            )
            .with_validator(RateValidator {})
            .with_help_message("From 0 to 99%. Push Enter if the modifier doesn't exist.")
            .prompt()
            .handle_exit_signals()
            .map_err(|source| EnvironmentError::SpecificInputError {
                description: "job duration modifier".to_string(),
                source,
            })?;
            let job_duration_modifier = match job_duration_modifier {
                Some(job_duration_modifier) => job_duration_modifier,
                None => return Ok(true),
            };
            let job_duration_modifier = if job_duration_modifier.is_empty() {
                None
            } else {
                Some(job_duration_modifier.parse::<f64>().unwrap() / 100.0)
            };

            let industry_type = IndustryType::from_str(&industry_type)?;
            activities.insert(
                industry_type,
                PlayerStructureStats {
                    tax_rate,
                    job_duration_modifier,
                    job_cost_modifier,
                    material_consumption_modifier,
                },
            );
        }
    }
    let structure = PlayerStructure {
        id: structure.structure.id(),
        usages: facility_usages,
        activities,
    };
    eve.fs()
        .add_structure(&structure)
        .await
        .map_err(|source| ModelError::SaveStructureError { source })?;
    Ok(false)
}

async fn load_systems(eve: &DataIntegrator, region: &Region) -> Result<Vec<SolarSystem>, EveError> {
    let mut futures = vec![];
    for constellation_id in region.region.constellation_ids.clone() {
        futures.push(async move {
            let cons = eve.load_constellation(constellation_id).await?;
            Ok::<Constellation, DataLoadError>(cons)
        });
    }
    let all_operations = futures.into_iter().collect::<TryJoinAll<_>>();
    let constellations = try_join!(all_operations)?.0;

    let mut futures = vec![];
    for constellation in constellations {
        for system_id in constellation.system_ids.clone() {
            futures.push(async move {
                let system = eve.load_system(system_id).await?;
                Ok::<SolarSystem, SystemLoadingError>(system)
            });
        }
    }
    let all_operations = futures.into_iter().collect::<TryJoinAll<_>>();
    let systems = try_join!(all_operations)?.0;
    Ok(systems)
}

struct Region {
    region: crate::model::locations::Region,
}

impl Display for Region {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.region.name())
    }
}

struct System {
    system: SolarSystem,
}

impl Display for System {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.system.name())
    }
}

struct Station {
    station: Facility,
}

impl Display for Station {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.station.name())
    }
}

struct Structure {
    structure: Facility,
}

impl Display for Structure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.structure.name())
    }
}

#[derive(Clone)]
struct RateValidator {}

impl StringValidator for RateValidator {
    fn validate(&self, input: &str) -> Result<Validation, CustomUserError> {
        match input.parse::<f64>() {
            Ok(nb) => {
                if nb <= 0.0 {
                    Ok(Validation::Invalid(ErrorMessage::Custom(
                        "Input is too low.".to_string(),
                    )))
                } else if nb >= 100.0 {
                    Ok(Validation::Invalid(ErrorMessage::Custom(
                        "Input is too high.".to_string(),
                    )))
                } else {
                    Ok(Validation::Valid)
                }
            }
            Err(_) => Ok(Validation::Invalid(ErrorMessage::Custom(
                "Input is not a number.".to_string(),
            ))),
        }
    }
}
