use std::env;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::authentication::RefreshToken;
use crate::configuration::{Configuration, ConfigurationError};

use super::ConfigurationDirectoryType;

#[derive(Debug, Error, PartialEq)]
#[error("Could not read value of environment variable {name}: {source}")]
pub struct UnreadableVarError {
    name: String,
    source: env::VarError,
}

fn var(name: &str) -> Result<Option<String>, UnreadableVarError> {
    match std::env::var(name) {
        Ok(val) => Ok(Some(val)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(UnreadableVarError {
            name: name.to_string(),
            source: e,
        }),
    }
}

fn with_prefix(base_name: &str) -> String {
    format!("EVEVULCAIN_{}", base_name)
}

pub fn get_directory(
    dir_type: &ConfigurationDirectoryType,
) -> Result<Option<PathBuf>, UnreadableVarError> {
    let dir = match dir_type {
        ConfigurationDirectoryType::Data => var(&with_prefix("DATA_DIR"))?,
        ConfigurationDirectoryType::Cache => var(&with_prefix("CACHE_DIR"))?,
        ConfigurationDirectoryType::Configuration => match var(&with_prefix("CONFIG"))? {
            Some(cfg_file) => {
                let file = Path::new(&cfg_file);
                let x = match file.parent() {
                    Some(parent_path) => parent_path.to_str().map(|p| p.to_string()),
                    None => None,
                };
                x
            }
            None => None,
        },
    };
    if let Some(path) = dir {
        return Ok(Some(PathBuf::from(path)));
    }
    Ok(None)
}

pub fn get_config_file() -> Result<Option<PathBuf>, UnreadableVarError> {
    if let Some(cfg_file) = var(&with_prefix("CONFIG"))? {
        return Ok(Some(PathBuf::from(cfg_file)));
    }
    Ok(None)
}

pub struct EnvironmentVariablesConfiguration<T: Configuration> {
    default: T,
}

impl<T: Configuration> EnvironmentVariablesConfiguration<T> {
    pub fn new(default: T) -> Self {
        EnvironmentVariablesConfiguration { default }
    }
}

impl<T: Configuration> Configuration for EnvironmentVariablesConfiguration<T> {
    fn refresh_token(&self) -> Result<Option<RefreshToken>, ConfigurationError> {
        if let Some(t) = var(&with_prefix("REFRESH_TOKEN"))? {
            return Ok(Some(t));
        }
        self.default.refresh_token()
    }

    fn no_color(&self) -> Result<Option<bool>, ConfigurationError> {
        let var_name = with_prefix("NO_COLOR");
        if let Some(val) = var(&var_name)? {
            let json_output = if val == "1" {
                true
            } else if val == "0" {
                false
            } else {
                return Err(ConfigurationError::InvalidValueError {
                    got: val,
                    expected: "[0,1]".to_string(),
                    origin: format!("env var '{}'", var_name),
                });
            };
            return Ok(Some(json_output));
        }
        self.default.no_color()
    }

    fn api_client_id(&self) -> Result<String, ConfigurationError> {
        if let Some(id) = var(&with_prefix("API_CLIENT_ID"))? {
            return Ok(id);
        }
        self.default.api_client_id()
    }

    fn api_callback_url(&self) -> Result<String, ConfigurationError> {
        if let Some(callback_url) = var(&with_prefix("API_CALLBACK_URL"))? {
            return Ok(callback_url);
        }
        self.default.api_callback_url()
    }

    fn base_api_url(&self) -> Result<Option<String>, ConfigurationError> {
        if let Some(base_api_url) = var(&with_prefix("BASE_API_URL"))? {
            return Ok(Some(base_api_url));
        }
        self.default.base_api_url()
    }

    fn authorize_url(&self) -> Result<Option<String>, ConfigurationError> {
        if let Some(authorize_url) = var(&with_prefix("AUTHORIZE_URL"))? {
            return Ok(Some(authorize_url));
        }
        self.default.authorize_url()
    }

    fn token_url(&self) -> Result<Option<String>, ConfigurationError> {
        if let Some(token_url) = var(&with_prefix("TOKEN_URL"))? {
            return Ok(Some(token_url));
        }
        self.default.token_url()
    }

    fn spec_url(&self) -> Result<Option<String>, ConfigurationError> {
        if let Some(spec_url) = var(&with_prefix("SPEC_URL"))? {
            return Ok(Some(spec_url));
        }
        self.default.spec_url()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_var_present() {
        let var_name = "EXISTING_VAR";
        let var_value = "some_value";
        std::env::set_var(var_name, var_value);

        let result = var(var_name);

        assert_eq!(result, Ok(Some(var_value.to_string())));
    }

    #[test]
    fn test_var_not_present() {
        // Arrange
        let var_name = "NON_EXISTING_VAR";
        std::env::remove_var(var_name);

        // Act
        let result = var(var_name);

        // Assert
        assert_eq!(result, Ok(None));
    }
}
