use std::io;
use std::num::ParseIntError;
use std::sync::mpsc::RecvError;

use colored::ColoredString;
use rfesi::prelude::EsiError;
use thiserror::Error;
use url::Url;
use zip::result::ZipError;

use crate::api::evecache::cache::CacheError;
use crate::authentication::tokens::TokenError;
use crate::cache::{FSCacheReadError, FSCacheWriteError};
use crate::configuration::{
    ConfigurationError, ConfigurationInitializationError, FSRessourcesError,
};
use crate::filesystem::DirectoryCreationError;
use crate::http::HttpInitError;

use crate::integration::{
    DataLoadError, FacilityLoadingError, ItemLoadingError, SystemLoadingError,
};
use crate::logging::StdoutError;
use crate::model::facility::manufacture::ManufactureError;
use crate::model::facility::markets::{MarketError, VolumesError};
use crate::model::facility::IdentifierTypeConversionFailed;
use crate::vector::UnicityError;
use crate::{authentication, configuration, filesystem, http};

pub trait Advice {
    fn advice(&self) -> Option<ColoredString>;
}

#[derive(Debug, Error)]
pub enum EveError {
    #[error(transparent)]
    FSRessourcesError(#[from] FSRessourcesError),
    #[error(transparent)]
    EnvironmentError(#[from] EnvironmentError),
    #[error(transparent)]
    DirectoryCreationError(#[from] DirectoryCreationError),
    #[error(transparent)]
    ConfigurationInitializationError(#[from] ConfigurationInitializationError),
    #[error(transparent)]
    FSCacheWriteError(#[from] FSCacheWriteError),
    #[error(transparent)]
    DataLoadError(#[from] DataLoadError),
    #[error(transparent)]
    ModelError(#[from] ModelError),
    #[error(transparent)]
    IdentifierTypeConversionFailed(#[from] IdentifierTypeConversionFailed),
    #[error(transparent)]
    FacilityLoadingError(#[from] FacilityLoadingError),
    #[error(transparent)]
    StrumParseError(#[from] strum::ParseError),
    #[error(transparent)]
    SystemLoadingError(#[from] SystemLoadingError),
    #[error(transparent)]
    StdoutError(#[from] StdoutError),
    #[error(transparent)]
    UnicityError(#[from] UnicityError),
    #[error(transparent)]
    ItemLoadingError(#[from] ItemLoadingError),
    #[error(transparent)]
    EveAuthenticationError(#[from] EveAuthenticationError),
    #[error(transparent)]
    EveApiError(#[from] EveApiError),
    #[error(transparent)]
    HTTPServerError(#[from] HTTPServerError),
    #[error(transparent)]
    ManufactureError(#[from] ManufactureError),
    #[error(transparent)]
    VolumesError(#[from] VolumesError),
    #[error(transparent)]
    MarketError(#[from] MarketError),
}

impl Advice for EveError {
    // Planned for future error handling and user feedback on which command to call to correct an error.
    #[allow(clippy::match_single_binding)]
    fn advice(&self) -> Option<ColoredString> {
        match self {
            _ => None,
        }
    }
}

#[derive(Error, Debug)]
pub enum EnvironmentError {
    #[error("Error when reading input '{description}': {source}")]
    SpecificInputError {
        description: String,
        source: inquire::InquireError,
    },
    #[error("Cache directory unknown")]
    CacheDirectoryUnknown,
    #[error("Data directory unknown")]
    DataDirectoryUnknown,
    #[error("Configuration directory unknown")]
    ConfigurationDirectoryUnknown,
    #[error("Could not load configuration '{option_name}': {source}")]
    ConfigurationOptionLoading {
        option_name: String,
        source: configuration::ConfigurationError,
    },
    #[error("Configuration path could not be determined.")]
    ConfigurationFilePathNotFound,
    #[error("Could not serialize configuration to TOML format: {source}")]
    TOMLConfigurationSerilizationError { source: toml::ser::Error },
    #[error("Could not write TOML configuration to file '{path}': {source}")]
    TOMLConfigurationWriteError { path: String, source: io::Error },
    #[error("Could open browser on URL '{url}': {source}")]
    BrowserOpening { url: String, source: io::Error },
    #[error("Could not read from stdin: {source}")]
    STDInReadFailed { source: io::Error },
    #[error("Error when trying to parse port for HTTP Server from '{port}'")]
    ParsePortError { port: String, source: ParseIntError },
}

#[derive(Error, Debug)]
pub enum EveApiError {
    #[error("Could not download SDE: {source}")]
    SDEDownloadFailed { source: reqwest::Error },
    #[error("SDE Directory could not be created ({path}): {source}")]
    SDEDirectoryCreationFailed { path: String, source: io::Error },
    #[error("SDE File could not be created: {source}")]
    SDEFileCreationFailed { source: io::Error },
    #[error("SDE File could not be written to destination ({path}): {source}")]
    SDEFileWriteFailed { path: String, source: io::Error },
    #[error("Could not read downloaded SDE: {source}")]
    SDEBodyReadFailed { source: reqwest::Error },
    #[error("Could not open SDE zip file: {source}")]
    SDEZipOpenError { source: io::Error },
    #[error("Could not open SDE zip archive: {source}")]
    SDEZipOpenArchival { source: ZipError },
    #[error("Could not open file in SDE archive: {source}")]
    SDEZipInternalFileOpenError { source: ZipError },
    #[error("Could not read file in SDE archive: {source}")]
    SDEZipInternalFileReadError { source: io::Error },
    #[error("Could not write read content from zip archive to file: {source}")]
    SDEZipWriteReadContentToFile { source: io::Error },
    #[error("Could not delete previous SDE file: {source}")]
    SDEDeletePreviousZip { source: io::Error },
    #[error("Could not check existence of SDE file '{path}': {source}")]
    SDEEFileExistenceCheckFailed {
        path: String,
        source: std::io::Error,
    },
    #[error("Could not split larde SDE file into subfiles: {source}")]
    SDESplitFile { source: filesystem::SplitError },
    #[error("Could not initialize ESI builder: {source}")]
    ESIBuilderInitError { source: ConfigurationError },
    #[error("Could not initialize ESI: {source}")]
    ESIInitFailed { source: EsiError },
    #[error("Could not initialize Cache: {source}")]
    ESICacheInitFailed { source: FSCacheReadError },
}

#[derive(Debug, Error)]
pub enum EveAuthenticationError {
    #[error("Access token not found")]
    AccessTokenNotFound,
    #[error("Refresh token not found")]
    RefreshTokenNotFound,
    #[error("Authentication URL returned by esi is empty: {url}")]
    ESIAuthURLEmptyQuery { url: Url },
    #[error("State returned by OAuth ({got}) doesn't correspond to expected one ({expected})")]
    ReturnedStateDoesNotCorrespond { expected: String, got: String },
    #[error("Access token claims not found")]
    TokenClaimsNotFound,
    #[error("Could not decode token: {source}")]
    TokenDecodingFailed { source: TokenError },
    #[error("Could not delete refresh token: {source}")]
    RefreshTokenDelete { source: filesystem::FSDeleteError },
    #[error("Could not receive URL from channel: {source}")]
    ReceivingCodeURLFailed { source: RecvError },
    #[error("Could not verify received token: {source}")]
    TokenVerificationFailed { source: EsiError },
    #[error("Could not persist refresh token: {source}")]
    RefreshTokenPersistingError {
        source: crate::filesystem::FSWriteError,
    },
    #[error(transparent)]
    AuthenticationLoadingError {
        source: authentication::AuthenticationError,
    },
}

#[derive(Debug, Error)]
pub enum HTTPServerError {
    #[error("Could not initialize HTTP Server on port {port}: {source}")]
    HTTPServerInitialization { port: u32, source: HttpInitError },
    #[error("HTTP Server could not listen on port {port}: {source}")]
    HTTPServerListening {
        port: u32,
        source: http::HTTPServerError,
    },
}

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("Search success but item not found: {name}")]
    NoItemFound { name: String },
    #[error("Blueprint missing from SDE for '{name}' ({type_id})")]
    BlueprintMissing { name: String, type_id: i32 },
    #[error("No station found in {region_name} > {system_name}")]
    NoStationFoundInSystem {
        system_name: String,
        region_name: String,
    },
    #[error("Error loading stations from {system_name} ({system_id}): {source}")]
    LoadStationInSystemError {
        system_id: i32,
        system_name: String,
        source: crate::integration::FacilityLoadingError,
    },
    #[error("Error saving station: {source}")]
    SaveStationError { source: crate::filesystem::FSError },
    #[error("Error saving structure: {source}")]
    SaveStructureError { source: crate::filesystem::FSError },
    #[error("Error saving item: {source}")]
    SaveItemError { source: crate::filesystem::FSError },
    #[error("Searched structure not found: '{search}'")]
    SearchedStructureNotFound { search: String },
    #[error(transparent)]
    LoadingCharacter {
        source: crate::integration::CharacterLocationError,
    },
    #[error(transparent)]
    LoadingFacilities {
        source: crate::integration::FacilityLoadingError,
    },
    #[error("Could not load blueprints: {source}")]
    LoadBlueprints { source: CacheError },
    #[error(transparent)]
    LoadingBlueprint {
        source: crate::integration::DataLoadError,
    },
    #[error(transparent)]
    LoadingPrices {
        source: crate::integration::DataLoadError,
    },
    #[error("Searched item not found: '{search}'")]
    SearchedItemNotFound { search: String },
    #[error("Could not remove NPC station: {source}")]
    RemovingNPCStation { source: filesystem::FSError },
    #[error("Could not remove player structure: {source}")]
    RemovingPlayerStructure { source: filesystem::FSError },
    #[error("Could not remove item: {source}")]
    RemovingItem { source: filesystem::FSError },
    #[error("Too much blueprints found for {name} ({type_id})")]
    TooMuchBlueprint { name: String, type_id: i32 },
    #[error("the blueprint doesn't have any invention info: {blueprint_id}")]
    IsNotAnInventionBlueprint { blueprint_id: i32 },
    #[error("Loading skill '{skill_id}': {source}")]
    LoadSkill {
        skill_id: i32,
        source: crate::api::evecache::cache::CacheError,
    },
    #[error(transparent)]
    LoadingIndustryJobs {
        source: crate::integration::IndustryJobsLoadingError,
    },
    #[error(transparent)]
    LoadingCharacterOrders {
        source: crate::integration::DataLoadError,
    },
}
