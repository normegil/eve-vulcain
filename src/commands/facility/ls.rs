use std::collections::HashMap;

use colored::{ColoredString, Colorize};
use serde::Serialize;
use strum::IntoEnumIterator;

use crate::{
    errors::EveError,
    integration::DataIntegrator,
    logging::{self, Message, Stdout, Verbosity},
    model::{
        common::{Identified, Named},
        facility::{Facility, FacilityType, FacilityUsage},
        industry::IndustryType,
    },
};

pub async fn ls(eve: &DataIntegrator) -> Result<(), EveError> {
    let facilities = eve.load_registered_facilities().await?;
    logging::stdoutln(FacilityLSStdout::from(facilities))?;
    Ok(())
}

#[derive(Serialize)]
struct FacilityLSStdout {
    markets: Vec<IndustrialFacilityStdout>,
    industrial_facilities: IndustrialFacilities,
}

impl Stdout for FacilityLSStdout {}

impl FacilityLSStdout {
    pub fn from(facilities: Vec<Facility>) -> Self {
        let markets = facilities
            .iter()
            .filter(|f| f.registered_usages.is_some())
            .filter(|f| {
                f.registered_usages
                    .as_ref()
                    .expect("None usages filtered at previous step")
                    .contains(&FacilityUsage::Market)
            })
            .map(IndustrialFacilityStdout::from)
            .collect();
        let industrial_facilities = IndustrialFacilities::from(&facilities);

        Self {
            markets,
            industrial_facilities,
        }
    }
}

impl Message for FacilityLSStdout {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let mut all_markets = String::new();
        for market in &self.markets {
            all_markets += market.standard(verbosity).to_string().as_str();
        }
        let markets = format!("Markets:\n{}", all_markets);

        let industrial_facilities_str = self.industrial_facilities.standard(verbosity);
        ColoredString::from(format!("{markets}\n{industrial_facilities_str}").as_str())
    }
}

#[derive(Serialize)]
pub struct IndustrialFacilityStdout {
    name: String,
}

impl IndustrialFacilityStdout {
    pub fn from(facility: &Facility) -> Self {
        Self {
            name: facility.name(),
        }
    }
}

impl Message for IndustrialFacilityStdout {
    fn standard(&self, _: Verbosity) -> ColoredString {
        ColoredString::from(format!("\t\t> {}\n", self.name.bold()).as_str())
    }
}

#[derive(Serialize)]
pub struct IndustrialFacilities {
    facilities: HashMap<IndustryType, Vec<IndustrialFacilityStdout>>,
}

impl IndustrialFacilities {
    pub fn from(facilities_in: &Vec<Facility>) -> Self {
        let mut facilities = HashMap::new();
        for industry_type in IndustryType::iter() {
            facilities.insert(industry_type, Vec::new());
        }

        for facility in facilities_in {
            if facility.registered_usages.is_none()
                || !facility
                    .registered_usages
                    .as_ref()
                    .expect("Checked at previous condition")
                    .contains(&FacilityUsage::Industry)
            {
                logging::debug!(
                    "Ignored facility for industry: {} (Usages:{:?})",
                    facility.name(),
                    facility.registered_usages
                );
                continue;
            }

            for industry_type in IndustryType::iter() {
                let facilities_per_industry = facilities
                    .get_mut(&industry_type)
                    .expect("Key should have been preinserted");
                match facility.facility_type() {
                    FacilityType::Station(_) => {
                        facilities_per_industry.push(IndustrialFacilityStdout::from(facility))
                    }
                    FacilityType::Structure(s) => {
                        logging::info!("Structure: {}", s.id());
                        if s.activities.contains_key(&industry_type) {
                            facilities_per_industry.push(IndustrialFacilityStdout::from(facility))
                        }
                    }
                }
            }
        }

        Self { facilities }
    }
}

impl Message for IndustrialFacilities {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        let mut out = String::new();
        for (industry_type, facilities) in &self.facilities {
            out += format!("\t{}:\n", industry_type).as_str();
            for facility in facilities {
                out += facility.standard(verbosity).to_string().as_str();
            }
        }
        ColoredString::from(format!("Industry:\n{out}").as_str())
    }
}
