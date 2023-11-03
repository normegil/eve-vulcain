use crate::authentication::RefreshToken;
use crate::configuration::{Configuration, ConfigurationError};

pub struct DefaultConfiguration;

impl Configuration for DefaultConfiguration {
    fn refresh_token(&self) -> Result<Option<RefreshToken>, ConfigurationError> {
        Ok(None)
    }

    fn no_color(&self) -> Result<Option<bool>, ConfigurationError> {
        Ok(None)
    }

    fn api_client_id(&self) -> Result<String, ConfigurationError> {
        Ok("e7f2e5f9a5474ec7b08e9fb249bc62d9".to_string())
    }

    fn api_callback_url(&self) -> Result<String, ConfigurationError> {
        Ok("http://localhost:54621/".to_string())
    }

    fn base_api_url(&self) -> Result<Option<String>, ConfigurationError> {
        Ok(None)
    }

    fn authorize_url(&self) -> Result<Option<String>, ConfigurationError> {
        Ok(None)
    }

    fn token_url(&self) -> Result<Option<String>, ConfigurationError> {
        Ok(None)
    }

    fn spec_url(&self) -> Result<Option<String>, ConfigurationError> {
        Ok(None)
    }
}
