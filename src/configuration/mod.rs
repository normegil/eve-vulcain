use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

use crate::authentication::RefreshToken;
use crate::configuration::cli::{Args, CLIConfiguration};
use crate::configuration::default::DefaultConfiguration;
use crate::configuration::env::EnvironmentVariablesConfiguration;
use crate::configuration::files::{FSConfiguration, FSConfigurationPaths};

use crate::filesystem;

pub mod cli;

pub mod env;

pub mod files;

pub mod default;

#[derive(Clone, Debug)]
pub enum ConfigurationDirectoryType {
    Data,
    Cache,
    Configuration,
}

impl std::fmt::Display for ConfigurationDirectoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ConfigurationDirectoryType::Data => "Data",
            ConfigurationDirectoryType::Cache => "Cache",
            ConfigurationDirectoryType::Configuration => "Configuration",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Error)]
pub enum ConfigurationInitializationError {
    #[error(transparent)]
    SystemError {
        #[from]
        source: Arc<dyn std::error::Error + Send + Sync>,
    },
}

pub async fn get(args: &Args) -> Result<impl Configuration, ConfigurationInitializationError> {
    let cfg_dirs = FSConfigurationPaths {
        data_dir: get_directory(ConfigurationDirectoryType::Data, args)
            .await
            .map_err(Arc::from)
            .map_err(|e| ConfigurationInitializationError::SystemError { source: e })?,
        cfg_file: get_config_file(args)
            .await
            .map_err(Arc::from)
            .map_err(|e| ConfigurationInitializationError::SystemError { source: e })?,
    };
    let cfg = FSConfiguration::new(cfg_dirs, DefaultConfiguration)
        .map_err(Arc::from)
        .map_err(|e| ConfigurationInitializationError::SystemError { source: e })?;
    let env_var_cfg = EnvironmentVariablesConfiguration::new(cfg);
    Ok(CLIConfiguration::new(args, env_var_cfg))
}

#[derive(Debug, Error)]
pub enum FSRessourcesError {
    #[error(transparent)]
    EnvVarError {
        #[from]
        source: env::UnreadableVarError,
    },
}

pub async fn get_config_file(args: &Args) -> Result<Option<PathBuf>, FSRessourcesError> {
    if let Some(cfg_path) = args.config.clone() {
        return Ok(Some(cfg_path));
    } else if let Some(cfg_path) = env::get_config_file()? {
        return Ok(Some(cfg_path));
    } else if let Some(mut cfg_dir) =
        files::get_directory(&ConfigurationDirectoryType::Configuration).await
    {
        cfg_dir.push("eve-vulcain.toml");
        return Ok(Some(cfg_dir));
    }
    Ok(None)
}

pub async fn get_directory(
    dir_type: ConfigurationDirectoryType,
    args: &Args,
) -> Result<Option<PathBuf>, FSRessourcesError> {
    let cache_dir = if let Some(dir) = cli::get_directory(args, &dir_type) {
        Some(dir)
    } else if let Some(dir) = env::get_directory(&dir_type)? {
        Some(dir)
    } else {
        files::get_directory(&dir_type).await
    };
    Ok(cache_dir)
}

#[derive(Error, Debug)]
pub enum ConfigurationError {
    #[error(transparent)]
    EnvVarError(#[from] env::UnreadableVarError),
    #[error(transparent)]
    FSReadError(#[from] filesystem::FSReadError),
    #[error("value '{got}' is not valid ('{expected}') - loaded from {origin}")]
    InvalidValueError {
        got: String,
        expected: String,
        origin: String,
    },
    #[error("Unsupported cache level: {specified}")]
    InvalidCacheLevel { specified: String },
}

pub trait Configuration {
    fn refresh_token(&self) -> Result<Option<RefreshToken>, ConfigurationError>;

    fn no_color(&self) -> Result<Option<bool>, ConfigurationError>;

    fn api_client_id(&self) -> Result<String, ConfigurationError>;

    fn api_callback_url(&self) -> Result<String, ConfigurationError>;

    fn base_api_url(&self) -> Result<Option<String>, ConfigurationError>;

    fn authorize_url(&self) -> Result<Option<String>, ConfigurationError>;

    fn token_url(&self) -> Result<Option<String>, ConfigurationError>;

    fn spec_url(&self) -> Result<Option<String>, ConfigurationError>;
}
