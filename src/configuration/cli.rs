use std::path::PathBuf;
use std::str::FromStr;

use clap::{Parser, Subcommand};

use crate::authentication::RefreshToken;
use crate::configuration;
use crate::configuration::{Configuration, ConfigurationError};

use super::ConfigurationDirectoryType;

/// EVE Online Industry Tool
///
/// Compute costs and profits of various industry operations in EVE Online.
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to cache directory, which store all caches for ESI tokens and requests. The directory will be created if it doesn't exist.
    #[arg(global = true, long)]
    pub cache_directory: Option<PathBuf>,
    /// Path to data directory, which store registered facilities and items. The directory will be created if it doesn't exist.
    #[arg(global = true, long)]
    pub data_directory: Option<PathBuf>,
    /// Path to configuration file.
    #[arg(global = true, long)]
    pub config: Option<PathBuf>,
    /// Disable all output formating options.
    #[arg(global = true, long)]
    pub no_color: bool,
    /// Force enabling formating options, useful in context where it wouldn't be supported (such as piping to another command).
    #[arg(global = true, long)]
    pub force_color: bool,
    /// Disable all logging and most normal output.
    #[arg(global = true, short, long)]
    pub quiet: bool,
    /// Force the client ID for ESI authentication
    #[arg(global = true, long)]
    pub client_id: Option<String>,
    /// Force the callback URL for ESI authentication
    #[arg(global = true, long)]
    pub callback_url: Option<String>,
    /// Set the level of caching (available values: full, memory, disabled )
    #[arg(global = true, long, default_value = "full")]
    pub cache_level: CacheLevel,
    /// Set verbosity level ('v', 'vv' or 'vvv')
    #[arg(global = true, short = 'v', action = clap::ArgAction::Count)]
    pub verbose: u8,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, PartialEq, Clone)]
pub enum Commands {
    /// Setup environment for eve-vulcain
    Init,
    /// Install/Update SDE data. Useless if the 'Init' command was run.
    Update(UpdateOptions),
    /// Login to eve online  
    Login,
    /// Logout from eve online
    Logout,
    /// Display the state of the current logged in character, with it's location, market orders & current jobs
    State(StateOptions),
    /// Compute costs and profits linked to manufacturing items
    Manufacture(ManufacturingOptions),
    /// Compute costs linked to invention of tech 2 blueprints and items
    Invent(InventionOptions),
    /// Manage registered markets and industry facilities
    Facility(FacilityOptions),
    /// Manage registered items
    Item(ItemOptions),
}

#[derive(clap::Args, Debug, PartialEq, Clone)]
pub struct StateOptions {
    /// Generate command result and details as JSON output on stdout.
    #[arg(global = true, long)]
    pub json: bool,
}

#[derive(clap::Args, Debug, PartialEq, Clone)]
pub struct UpdateOptions {
    /// Prevent the download of a new SDE archive.
    #[arg(long)]
    pub no_download: bool,
}

#[derive(clap::Args, Debug, PartialEq, Clone)]
pub struct InventionOptions {
    /// Generate command result and details as JSON output on stdout.
    #[arg(global = true, long)]
    pub json: bool,
    #[command(subcommand)]
    pub command: InventionCommands,
}

#[derive(Subcommand, Debug, PartialEq, Clone)]
pub enum InventionCommands {
    /// Compute invention cost related to a specific tech 2 item
    Item(InventionItemOptions),
}

#[derive(clap::Args, Debug, PartialEq, Clone)]
pub struct InventionItemOptions {
    /// Partial or full name of an item (not a blueprint!) to invent
    #[clap(index = 1)]
    pub item: String,
    /// If specified, the item name will be searched for exact match
    #[arg(long)]
    pub strict: bool,
}

#[derive(clap::Args, Debug, PartialEq, Clone)]
pub struct ManufacturingOptions {
    /// Generate command result and details as JSON output on stdout.
    #[arg(global = true, long)]
    pub json: bool,
    #[command(subcommand)]
    pub command: ManufacturingCommands,
    /// Force a specific blueprint material efficiency to compute the manufacturing cost of an item (Max: 10)
    #[arg(long, global = true, default_value = "0")]
    pub material_efficiency: u8,
    /// Force a specific blueprint time efficiency to compute the manufacturing cost of an item (Max: 20)
    #[arg(long, global = true, default_value = "0")]
    pub time_efficiency: u8,
}

#[derive(Subcommand, Debug, PartialEq, Clone)]
pub enum ManufacturingCommands {
    /// Compute the profits of all items and sort them (ISK/h)
    All(ManufactureAllOptions),
    /// Compute the costs, volumes and profits of a specified item
    Item(MultipleItemsOptions),
}

#[derive(clap::Args, Debug, PartialEq, Clone)]
pub struct ManufactureAllOptions {
    /// Experimental. Instead of computing registered items, this will load all manufacturable items available in Eve Online. Takes a long time.
    #[arg(long)]
    pub everything: bool,
}

#[derive(clap::Args, Debug, PartialEq, Clone)]
pub struct MultipleItemsOptions {
    /// Partial or full name of an item to manufacture
    #[clap(index = 1)]
    pub item: String,
    /// If specified, the item name will be searched for exact match
    #[arg(long)]
    pub strict: bool,
}

#[derive(clap::Args, Debug, PartialEq, Clone)]
pub struct FacilityOptions {
    #[command(subcommand)]
    pub command: FacilityCommands,
}

#[derive(clap::Args, Debug, PartialEq, Clone)]
pub struct ItemOptions {
    #[command(subcommand)]
    pub command: ItemCommands,
}

#[derive(Subcommand, Debug, PartialEq, Clone)]
pub enum FacilityCommands {
    /// Add a facility to registered facilities
    Add(FacilityAddOptions),
    /// List all registered facilities
    Ls,
    /// Remove a registered facility
    Rm,
}

#[derive(Subcommand, Debug, PartialEq, Clone)]
pub enum ItemCommands {
    /// Add an item to registered items
    Add(ItemAddOptions),
    /// List all registered items
    Ls,
    /// Remove a registered item
    Rm,
}

#[derive(clap::Args, Debug, PartialEq, Clone)]
pub struct ItemAddOptions {
    /// Partial or full name of an item to add/search
    #[clap(index = 1)]
    pub item: Option<String>,
}

#[derive(clap::Args, Debug, PartialEq, Clone)]
pub struct FacilityAddOptions {}

#[derive(Clone, Debug)]
pub enum CacheLevel {
    Disabled,
    Memory,
    Full,
}

impl FromStr for CacheLevel {
    type Err = configuration::ConfigurationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "disabled" => Ok(CacheLevel::Disabled),
            "memory" => Ok(CacheLevel::Memory),
            "full" => Ok(CacheLevel::Full),
            s => Err(ConfigurationError::InvalidCacheLevel {
                specified: s.to_string(),
            }),
        }
    }
}

pub fn get_directory(args: &Args, dir_type: &ConfigurationDirectoryType) -> Option<PathBuf> {
    match dir_type {
        ConfigurationDirectoryType::Data => args.data_directory.clone(),
        ConfigurationDirectoryType::Cache => args.cache_directory.clone(),
        ConfigurationDirectoryType::Configuration => {
            if let Some(cfg) = &args.config {
                return cfg.parent().map(|p| p.to_path_buf());
            }
            None
        }
    }
}

pub struct CLIConfiguration<T: Configuration> {
    args: Args,
    default: T,
}

impl<T: Configuration> CLIConfiguration<T> {
    pub fn new(args: &Args, default: T) -> Self {
        CLIConfiguration {
            args: args.clone(),
            default,
        }
    }
}

impl<T: Configuration> Configuration for CLIConfiguration<T> {
    fn refresh_token(&self) -> Result<Option<RefreshToken>, ConfigurationError> {
        // Planned for future options
        #[allow(clippy::match_single_binding)]
        let token = match self.args.command {
            _ => None,
        };
        if let Some(token) = token {
            return Ok(Some(token));
        }
        self.default.refresh_token()
    }

    fn no_color(&self) -> Result<Option<bool>, ConfigurationError> {
        if self.args.no_color {
            return Ok(Some(true));
        }
        self.default.no_color()
    }

    fn api_client_id(&self) -> Result<String, ConfigurationError> {
        if let Some(id) = self.args.client_id.clone() {
            return Ok(id);
        }
        self.default.api_client_id()
    }

    fn api_callback_url(&self) -> Result<String, ConfigurationError> {
        if let Some(id) = self.args.callback_url.clone() {
            return Ok(id);
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
