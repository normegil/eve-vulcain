use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::Arc;

use colored::{ColoredString, Colorize};
use serde::Serialize;

use crate::authentication::tokens::TokenHelper;
use crate::authentication::{self, Authenticator};
use crate::configuration::Configuration;
use crate::errors::{
    EnvironmentError, EveApiError, EveAuthenticationError, EveError, HTTPServerError,
};
use crate::filesystem::FSData;
use crate::http::{HttpStatus, Request, Response, Server};
use crate::logging;
use crate::logging::{Message, Stdout, Verbosity};

pub async fn login<'a, C: Configuration>(
    authenticator: &Authenticator<'a, C>,
    api_client_id: String,
    persister: &FSData,
) -> Result<(), EveError> {
    let output = match authenticator.authenticate().await {
        Ok(esi) => {
            let access_token = esi
                .access_token
                .ok_or(EveAuthenticationError::AccessTokenNotFound)?;
            let token_data = TokenHelper { api_client_id }
                .decode(&access_token)
                .map_err(|source| EveAuthenticationError::TokenDecodingFailed { source })?;
            LoginOutput {
                name: token_data.claims.name.clone(),
                refresh_token: esi
                    .refresh_token
                    .ok_or(EveAuthenticationError::RefreshTokenNotFound)?,
                already_logged_in: true,
            }
        }
        Err(authentication::AuthenticationError::RefreshTokenNotFound) => {
            let mut esi = authenticator
                .esi_builder()
                .map_err(|source| EveApiError::ESIBuilderInitError { source })?
                .build()
                .map_err(|source| EveApiError::ESIInitFailed { source })?;
            let auth_infos = esi
                .get_authorize_url()
                .map_err(|source| EveApiError::ESIInitFailed { source })?;
            logging::info!("Open authorization URL: {}", auth_infos.authorization_url);
            open::that(&auth_infos.authorization_url).map_err(|source| {
                EnvironmentError::BrowserOpening {
                    url: auth_infos.authorization_url.clone(),
                    source,
                }
            })?;
            let (tx, rx) = channel();
            let server_port = 54621;
            logging::info!("Launch HTTP Server (port:{})", &server_port);

            let http_server = Server::new(
                server_port,
                Arc::new(move |req: Request| {
                    logging::debug!("Received authentication response: {}", &req.url);
                    if tx.send(req.url).is_err() {
                        let resp = Response {
                            status: HttpStatus::InternalServerError,
                            body: None,
                        };
                        return (resp, true);
                    }
                    let resp = Response {
                        status: HttpStatus::OK,
                        body: Some(String::from("Code received. You may close the current tab")),
                    };
                    logging::trace!("Sending response to browser");
                    (resp, true)
                }),
            )
            .map_err(|source| HTTPServerError::HTTPServerInitialization {
                port: server_port,
                source,
            })?;
            http_server
                .listen()
                .map_err(|source| HTTPServerError::HTTPServerListening {
                    port: server_port,
                    source,
                })?;

            let url = rx
                .recv()
                .map_err(|source| EveAuthenticationError::ReceivingCodeURLFailed { source })?;
            logging::info!("Received response from ESI");
            let query = url
                .query()
                .ok_or_else(|| EveAuthenticationError::ESIAuthURLEmptyQuery { url: url.clone() })?;
            let result = AuthenticationResult::from(query);
            logging::debug!("Parsed authentication result: {:?}", &result);
            if auth_infos.state != result.state {
                return Err(EveAuthenticationError::ReturnedStateDoesNotCorrespond {
                    expected: auth_infos.state,
                    got: result.state,
                })?;
            }
            logging::info!("Fetch authentication token");
            let claims = esi
                .authenticate(result.code.as_str(), auth_infos.pkce_verifier)
                .await
                .map_err(|source| EveAuthenticationError::TokenVerificationFailed { source })?
                .ok_or(EveAuthenticationError::TokenClaimsNotFound)?;
            logging::debug!("Authentication token received");
            let new_refresh_token = esi
                .refresh_token
                .ok_or(EveAuthenticationError::RefreshTokenNotFound)?;
            persister
                .save_refresh_token(&new_refresh_token)
                .await
                .map_err(|source| EveAuthenticationError::RefreshTokenPersistingError { source })?;
            LoginOutput {
                name: claims.name,
                refresh_token: new_refresh_token,
                already_logged_in: false,
            }
        }
        Err(source) => {
            return Err(EveAuthenticationError::AuthenticationLoadingError { source })?;
        }
    };
    logging::stdoutln(output)?;
    Ok(())
}

#[derive(Serialize)]
struct LoginOutput {
    #[serde(skip_serializing)]
    name: String,
    refresh_token: String,
    already_logged_in: bool,
}

impl Stdout for LoginOutput {}

impl Message for LoginOutput {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        if verbosity == Verbosity::Quiet {
            return ColoredString::from("");
        }

        let mut out = format!(
            "Connected as {}.\nRefresh token: {}",
            self.name.clone().bold(),
            self.refresh_token.clone().bold()
        );

        if self.already_logged_in {
            out = format!("{}\n\nYou were already logged in.\nTo reconnect with another character, please call '{}' before trying to login again.", out, "eve-vulcain logout".bold())
        }

        ColoredString::from(out.as_str())
    }
}

#[derive(Debug)]
struct AuthenticationResult {
    code: String,
    state: String,
}

impl AuthenticationResult {
    fn from(query: &str) -> Self {
        let mut args = HashMap::new();
        let split_query = query.split('&');
        for key_value in split_query {
            let mut splitted_key_value = key_value.split('=');
            let key = splitted_key_value.next().unwrap();
            let value = splitted_key_value.next().unwrap();
            args.insert(key, value);
        }
        AuthenticationResult {
            state: args["state"].to_string(),
            code: args["code"].to_string(),
        }
    }
}
