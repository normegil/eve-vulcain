use rfesi::prelude::{Esi, EsiBuilder, EsiError};
use thiserror::Error;

use crate::configuration::{Configuration, ConfigurationError};
use crate::filesystem::{FSData, FSWriteError};
use crate::{configuration, logging};

pub mod tokens;

#[derive(Debug, Error)]
pub enum AuthenticationError {
    #[error("Could not load configuration '{option_name}': {source}")]
    ConfigurationOptionLoading {
        option_name: String,
        source: configuration::ConfigurationError,
    },
    #[error("Could not initialize ESI: {source}")]
    ESIInitFailed { source: EsiError },
    #[error("Could not refresh access token using ESI: {source}")]
    RefreshAccessTokenFailed { source: EsiError },
    #[error("Could not refresh access token: {source}")]
    RefreshAccessTokenPersistError { source: FSWriteError },
    #[error("Refresh token not found in ESI response")]
    ResponseRefreshTokenNotFound,
    #[error("Refresh token not found in the local data")]
    RefreshTokenNotFound,
    #[error(transparent)]
    ConfigurationError {
        #[from]
        source: configuration::ConfigurationError,
    },
}

pub type RefreshToken = String;

pub struct Authenticator<'a, C: Configuration> {
    persister: &'a FSData,
    cfg: &'a C,
}

impl<'a, C: Configuration> Authenticator<'a, C> {
    pub fn new(persister: &'a FSData, cfg: &'a C) -> Self {
        Authenticator { persister, cfg }
    }

    pub fn esi_builder(&self) -> Result<EsiBuilder, ConfigurationError> {
        let mut builder = EsiBuilder::new()
        .user_agent("Eve-Vulcain")
        .client_id(&self.cfg.api_client_id()?)
        .callback_url(&self.cfg.api_callback_url()?)
        .scope("publicData esi-location.read_location.v1 esi-search.search_structures.v1 esi-universe.read_structures.v1 esi-skills.read_skills.v1 esi-wallet.read_character_wallet.v1 esi-industry.read_character_jobs.v1 esi-markets.read_character_orders.v1")
        .enable_application_authentication(true);
        if let Some(url) = self.cfg.base_api_url()? {
            logging::info!("Changing Base API URL: {}", url);
            builder = builder.base_api_url(&url)
        }
        if let Some(url) = self.cfg.authorize_url()? {
            logging::info!("Changing Authorize URL: {}", url);
            builder = builder.authorize_url(&url)
        }
        if let Some(url) = self.cfg.token_url()? {
            logging::info!("Changing Token URL: {}", url);
            builder = builder.token_url(&url)
        }
        if let Some(url) = self.cfg.spec_url()? {
            logging::info!("Changing Spec URL: {}", url);
            builder = builder.spec_url(&url)
        }
        Ok(builder)
    }

    pub async fn authenticate(&self) -> Result<Esi, AuthenticationError> {
        if let Some(token) = self.cfg.refresh_token().map_err(|source| {
            AuthenticationError::ConfigurationOptionLoading {
                option_name: "refresh_token".to_string(),
                source,
            }
        })? {
            let mut esi = self
                .esi_builder()?
                .refresh_token(Some(token.as_str()))
                .build()
                .map_err(|source| AuthenticationError::ESIInitFailed { source })?;
            esi.refresh_access_token(None)
                .await
                .map_err(|source| AuthenticationError::RefreshAccessTokenFailed { source })?;
            let new_refresh_token = esi
                .refresh_token
                .clone()
                .ok_or(AuthenticationError::ResponseRefreshTokenNotFound)?;
            self.persister
                .save_refresh_token(&new_refresh_token)
                .await
                .map_err(|source| AuthenticationError::RefreshAccessTokenPersistError { source })?;
            Ok(esi)
        } else {
            Err(AuthenticationError::RefreshTokenNotFound)
        }
    }
}
