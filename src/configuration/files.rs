use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::authentication::RefreshToken;
use crate::configuration::{Configuration, ConfigurationError};
use crate::display::Display;
use crate::filesystem::FSData;

use super::ConfigurationDirectoryType;

pub async fn get_directory(dir_type: &ConfigurationDirectoryType) -> Option<PathBuf> {
    let mut directory = match dir_type {
        ConfigurationDirectoryType::Data => dirs::data_local_dir(),
        ConfigurationDirectoryType::Cache => dirs::cache_dir(),
        ConfigurationDirectoryType::Configuration => dirs::config_dir(),
    };
    directory = directory.map(|mut d| {
        d.push("eve-vulcain");
        d
    });
    directory
}

#[derive(Debug, Error)]
pub enum FSConfigurationInitError {
    #[error("Could not load configuration file: {source}")]
    ConfigurationFileLoadingError {
        #[from]
        source: MainConfigurationInitError,
    },
    #[error("{dir_type} directory could not be determined but is required for configuration")]
    DirectoryUnknow {
        dir_type: ConfigurationDirectoryType,
    },
}

pub struct FSConfigurationPaths {
    pub data_dir: Option<PathBuf>,
    pub cfg_file: Option<PathBuf>,
}

pub struct FSConfiguration<T: Configuration> {
    data: FSData,
    cfg: Option<MainConfiguration>,
    default: T,
}

impl<T: Configuration> FSConfiguration<T> {
    pub fn new(dirs: FSConfigurationPaths, default: T) -> Result<Self, FSConfigurationInitError> {
        let cfg = MainConfiguration::from(dirs.cfg_file)?;
        Ok(FSConfiguration {
            data: FSData::new(
                dirs.data_dir
                    .ok_or(FSConfigurationInitError::DirectoryUnknow {
                        dir_type: ConfigurationDirectoryType::Data,
                    })?,
            ),
            cfg,
            default,
        })
    }
}

impl<T: Configuration> Configuration for FSConfiguration<T> {
    fn refresh_token(&self) -> Result<Option<RefreshToken>, ConfigurationError> {
        match self.data.load_refresh_token()? {
            None => self.default.refresh_token(),
            Some(s) => Ok(Some(s)),
        }
    }

    fn no_color(&self) -> Result<Option<bool>, ConfigurationError> {
        self.default.no_color()
    }

    fn api_client_id(&self) -> Result<String, ConfigurationError> {
        if let Some(cfg) = &self.cfg {
            if let Some(api) = &cfg.api {
                if let Some(id) = &api.client_id {
                    return Ok(id.to_string());
                }
            }
        }
        self.default.api_client_id()
    }

    fn api_callback_url(&self) -> Result<String, ConfigurationError> {
        if let Some(cfg) = &self.cfg {
            if let Some(api) = &cfg.api {
                if let Some(url) = &api.callback_url {
                    return Ok(url.to_string());
                }
            }
        }
        self.default.api_callback_url()
    }

    fn base_api_url(&self) -> Result<Option<String>, ConfigurationError> {
        self.default.base_api_url()
    }

    fn authorize_url(&self) -> Result<Option<String>, ConfigurationError> {
        self.default.authorize_url()
    }

    fn token_url(&self) -> Result<Option<String>, ConfigurationError> {
        self.default.token_url()
    }

    fn spec_url(&self) -> Result<Option<String>, ConfigurationError> {
        self.default.spec_url()
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct RefreshTokenStore {
    refresh_token: String,
}

#[derive(Deserialize, Serialize)]
pub struct MainConfiguration {
    pub authentication_server: Option<AuthenticationServerConfiguration>,
    pub api: Option<APIConfiguration>,
    pub facilities: Option<FacilitiesConfiguration>,
}

#[derive(Debug, Error)]
pub enum MainConfigurationInitError {
    #[error("Check existence of configuration file ({path}): {source}")]
    ExistenceCheck {
        path: String,
        source: std::io::Error,
    },
    #[error("Read configuration file ({path}): {source}")]
    ReadConfigurationFile {
        path: String,
        source: std::io::Error,
    },
    #[error("Coule not deserialize configuration file ({path}): {source}")]
    ConfigurationFileDeserialization {
        path: String,
        source: toml::de::Error,
    },
}

impl MainConfiguration {
    fn from(path: Option<PathBuf>) -> Result<Option<Self>, MainConfigurationInitError> {
        if let Some(path) = &path {
            if std::fs::try_exists(path).map_err(|e| {
                MainConfigurationInitError::ExistenceCheck {
                    path: path.to_display(),
                    source: e,
                }
            })? {
                let content = std::fs::read_to_string(path).map_err(|e| {
                    MainConfigurationInitError::ReadConfigurationFile {
                        path: path.to_display(),
                        source: e,
                    }
                })?;
                let result: MainConfiguration = toml::from_str(&content).map_err(|e| {
                    MainConfigurationInitError::ConfigurationFileDeserialization {
                        path: path.to_display(),
                        source: e,
                    }
                })?;
                Ok(Some(result))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct APIConfiguration {
    pub client_id: Option<String>,
    pub callback_url: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct AuthenticationServerConfiguration {
    pub port: Option<u16>,
}

#[derive(Deserialize, Serialize)]
pub struct FacilitiesConfiguration {
    pub facility: Vec<FacilityConfiguration>,
}

#[derive(Deserialize, Serialize)]
pub struct FacilityConfiguration {
    pub facility_type: FacilityType,
    pub facility_id: u64,
    pub services: Vec<FacilityService>,
}

#[derive(Deserialize, Serialize)]
pub enum FacilityType {
    Station,
    Structure,
}

#[derive(Deserialize, Serialize)]
pub struct FacilityService {
    pub service_type: FacilityServiceType,
    pub tax: u8,
}

#[derive(Deserialize, Serialize)]
pub enum FacilityServiceType {
    Manufacturing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_directory_data() {
        let dir_type = ConfigurationDirectoryType::Data;
        let result = get_directory(&dir_type).await;
        assert_eq!(
            result,
            dirs::data_local_dir().map(|mut d| {
                d.push("eve-vulcain");
                d
            })
        );
    }

    #[tokio::test]
    async fn test_get_directory_cache() {
        let dir_type = ConfigurationDirectoryType::Cache;
        let result = get_directory(&dir_type).await;
        assert_eq!(
            result,
            dirs::cache_dir().map(|mut d| {
                d.push("eve-vulcain");
                d
            })
        );
    }

    #[tokio::test]
    async fn test_get_directory_configuration() {
        let dir_type = ConfigurationDirectoryType::Configuration;
        let result = get_directory(&dir_type).await;
        assert_eq!(
            result,
            dirs::config_dir().map(|mut d| {
                d.push("eve-vulcain");
                d
            })
        );
    }
}
