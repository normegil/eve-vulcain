use chrono::Duration;
use colored::{ColoredString, Colorize};
use futures_util::future::try_join3;
use futures_util::TryFutureExt;
use serde::Serialize;

use crate::api::evecache::cache_keys::OrderType;
use crate::display::Display;
use crate::errors::{EveError, ModelError};
use crate::integration::DataIntegrator;
use crate::logging::{Message, Stdout, Verbosity};
use crate::model::character::{Character, CharacterLocation, Corporation};
use crate::model::common::Named;
use crate::model::facility::Facility;
use crate::model::industry::Job;
use crate::model::locations::{Constellation, Region, SolarSystem};
use crate::model::markets::CharacterOrder;

pub async fn state(eve: &DataIntegrator) -> Result<(), EveError> {
    let (character, jobs, orders) = try_join3(
        eve.load_character()
            .map_err(|source| ModelError::LoadingCharacter { source }),
        eve.load_character_industry_jobs()
            .map_err(|source| ModelError::LoadingIndustryJobs { source }),
        eve.load_character_orders()
            .map_err(|source| ModelError::LoadingCharacterOrders { source }),
    )
    .await?;

    crate::logging::stdoutln(StateStdout::from(character, jobs, orders))?;
    Ok(())
}

#[derive(Serialize)]
pub struct StateStdout {
    character_name: String,
    isk: f64,
    location: CharacterLocationStdout,
    corporation: CorporationStdout,
    jobs: JobsStdout,
    orders: OrdersStdout,
}

impl StateStdout {
    fn from(character: Character, jobs: Vec<Job>, orders: Vec<CharacterOrder>) -> Self {
        let orders: Vec<OrderStdout> = orders
            .into_iter()
            .map(|o| OrderStdout {
                item_name: o.item.name(),
                order_type: o.order_type,
                price: o.price,
                volume_remain: o.volume_remain,
                volume_total: o.volume_total,
            })
            .collect();

        let mut jobs: Vec<JobStdout> = jobs
            .into_iter()
            .filter(|j| j.item_produced.is_some())
            .map(|j| JobStdout {
                item_name: j.item_produced.as_ref().unwrap().name().clone(),
                runs: j.runs,
                duration_left: j.duration_left().num_seconds(),
            })
            .collect();

        jobs.sort_by(|a, b| a.duration_left.cmp(&b.duration_left));

        Self {
            character_name: character.name(),
            isk: character.isk,
            location: CharacterLocationStdout::from(&character.location),
            corporation: CorporationStdout::from(&character.corporation),
            jobs: JobsStdout { jobs },
            orders: OrdersStdout { orders },
        }
    }
}

#[derive(Serialize)]
pub struct JobsStdout {
    jobs: Vec<JobStdout>,
}

impl Message for JobsStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        if self.jobs.is_empty() {
            return ColoredString::from("");
        }
        let mut jobs_str = String::new();
        for job in &self.jobs {
            jobs_str += job.standard(verbosity).to_string().as_str();
        }
        ColoredString::from(format!("Jobs:\n{jobs_str}").as_str())
    }
}

#[derive(Serialize)]
pub struct JobStdout {
    item_name: String,
    runs: i32,
    duration_left: i64,
}

impl Message for JobStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        let duration_str = if self.duration_left == 0 {
            String::from("Done")
        } else {
            Duration::seconds(self.duration_left).to_display()
        };

        ColoredString::from(
            format!(
                "\t{:>40}{:>10} run(s) {:>20}\n",
                self.item_name.bold(),
                self.runs,
                duration_str
            )
            .as_str(),
        )
    }
}

#[derive(Serialize)]
pub struct OrdersStdout {
    orders: Vec<OrderStdout>,
}

impl Message for OrdersStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        if self.orders.is_empty() {
            return ColoredString::from("");
        }
        let mut orders_str = String::new();
        for order in &self.orders {
            orders_str += order.standard(verbosity).to_string().as_str();
        }
        ColoredString::from(format!("Market orders:\n{orders_str}").as_str())
    }
}

#[derive(Serialize)]
pub struct OrderStdout {
    item_name: String,
    order_type: OrderType,
    price: f64,
    volume_remain: i32,
    volume_total: i32,
}

impl Message for OrderStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        ColoredString::from(
            format!(
                "\t{:>40}{:>10}/{}{:>20} ISK/u\n",
                self.item_name.bold(),
                self.volume_remain.to_display(),
                self.volume_total.to_display(),
                self.price.to_display()
            )
            .as_str(),
        )
    }
}

#[derive(Serialize)]
pub struct Market {
    name: String,
}

impl Message for Market {
    fn standard(&self, _: Verbosity) -> ColoredString {
        ColoredString::from(format!("\t> {}\n", self.name.bold()).as_str())
    }
}

#[derive(Serialize)]
pub enum CharacterLocationStdout {
    Facility(FacilityStdout),
    Space(SystemStdout),
}

impl From<&CharacterLocation> for CharacterLocationStdout {
    fn from(value: &CharacterLocation) -> Self {
        match value {
            CharacterLocation::Facility(f) => {
                CharacterLocationStdout::Facility(FacilityStdout::from(f))
            }
            CharacterLocation::Space(s) => CharacterLocationStdout::Space(SystemStdout::from(s)),
        }
    }
}

impl Message for CharacterLocationStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        let solar_system = match &self {
            CharacterLocationStdout::Facility(f) => &f.system,
            CharacterLocationStdout::Space(s) => s,
        };

        let mut location = format!(
            "Location: {} > {} > {} ({:.3})",
            solar_system.constellation.region.name.bold(),
            solar_system.constellation.name.bold(),
            solar_system.name.bold(),
            match solar_system.security_status {
                x if x >= 0.5 => {
                    x.to_string().green()
                }
                x if (0.1..0.5).contains(&x) => {
                    x.to_string().yellow()
                }
                x => {
                    x.to_string().red()
                }
            }
        );

        if let CharacterLocationStdout::Facility(f) = self {
            location = format!("{} > {}", location, f.name.bold());
        }
        ColoredString::from(location.as_str())
    }
}

#[derive(Serialize)]
pub struct FacilityStdout {
    name: String,
    system: SystemStdout,
}

impl From<&Facility> for FacilityStdout {
    fn from(value: &Facility) -> Self {
        Self {
            name: value.name(),
            system: SystemStdout::from(&value.location),
        }
    }
}

#[derive(Serialize)]
pub struct SystemStdout {
    name: String,
    security_status: f64,
    constellation: ConstellationStdout,
}

impl From<&SolarSystem> for SystemStdout {
    fn from(value: &SolarSystem) -> Self {
        Self {
            name: value.name(),
            security_status: value.security_status,
            constellation: ConstellationStdout::from(&value.constellation),
        }
    }
}

#[derive(Serialize)]
pub struct ConstellationStdout {
    name: String,
    region: RegionStdout,
}

impl From<&Constellation> for ConstellationStdout {
    fn from(value: &Constellation) -> Self {
        Self {
            name: value.name(),
            region: RegionStdout::from(&value.region),
        }
    }
}

#[derive(Serialize)]
pub struct RegionStdout {
    name: String,
}

impl From<&Region> for RegionStdout {
    fn from(value: &Region) -> Self {
        Self { name: value.name() }
    }
}

#[derive(Serialize)]
pub struct CorporationStdout {
    name: String,
    alliance: Option<Alliance>,
}

impl From<&Corporation> for CorporationStdout {
    fn from(value: &Corporation) -> Self {
        Self {
            name: value.name(),
            alliance: value.alliance.as_ref().map(|a| Alliance { name: a.name() }),
        }
    }
}

impl Message for CorporationStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        let mut alliance = String::from("");
        if let Some(alliance_info) = self.alliance.as_ref() {
            alliance = format!("{} > ", alliance_info.name.bold());
        }
        ColoredString::from(format!("Corporation: {}{}", alliance, self.name.bold()).as_str())
    }
}

#[derive(Serialize)]
pub struct Alliance {
    name: String,
}

impl Message for StateStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let name = self.character_name.bold();
        let mut extra_info = String::from("");
        if verbosity != Verbosity::Quiet {
            let isk = format!("Wallet: {} {}", self.isk.to_display().bold(), "ISK".bold());

            let location = self.location.standard(verbosity);
            let corporation = self.corporation.standard(verbosity);

            let job = self.jobs.standard(verbosity);
            let orders = self.orders.standard(verbosity);

            extra_info = format!("{isk}\n{location}\n{corporation}\n\n{job}\n\n{orders}\n")
        }

        ColoredString::from(format!("{name}\n{extra_info}").as_str())
    }
}

impl Stdout for StateStdout {}
