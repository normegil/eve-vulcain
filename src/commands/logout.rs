use colored::{ColoredString, Colorize};
use serde::Serialize;

use crate::errors::{EveAuthenticationError, EveError};
use crate::filesystem::FSData;
use crate::logging;
use crate::logging::{Message, Stdout, Verbosity};

pub async fn logout(cache: &FSData) -> Result<(), EveError> {
    cache
        .delete_refresh_token()
        .await
        .map_err(|source| EveAuthenticationError::RefreshTokenDelete { source })?;
    logging::stdoutln(LogoutOutput)?;
    Ok(())
}

#[derive(Serialize)]
pub struct LogoutOutput;

impl Stdout for LogoutOutput {}

impl Message for LogoutOutput {
    fn standard(&self, verbosity: Verbosity) -> ColoredString {
        if verbosity == Verbosity::Quiet {
            return ColoredString::from("");
        }
        ColoredString::from(
            format!(
                "Successfully logged out.\nCall {} to restart login process.",
                "eve-vulcain login".bold()
            )
            .as_str(),
        )
    }
}
