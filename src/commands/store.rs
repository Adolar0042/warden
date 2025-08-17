use anyhow::{Context as _, Result, bail};
use tracing::{info, instrument, warn};

use crate::commands::common::styled_error_line;
use crate::config::OAuthConfig;
use crate::keyring::{Token, store_keyring_token};
use crate::utils::parse_credential_request;

#[instrument(skip(oauth_config))]
pub async fn handle_store(oauth_config: OAuthConfig) -> Result<()> {
    if oauth_config.oauth_only.is_some_and(|x| x) {
        return Ok(());
    }
    info!("Storing credentials...");
    let req = parse_credential_request().context("Failed to parse credential request")?;
    if let Some(credential) = &req.username
        && let Some(password) = &req.password
    {
        let token = Token::new(
            password.clone(),
            req.oauth_refresh_token,
            req.password_expiry_utc,
        );
        store_keyring_token(credential, &req.host, &token)
            .context("Failed to store token in keyring")?;
        Ok(())
    } else {
        let msg = "No username or password provided in request; nothing to store.";
        warn!("{msg}");
        eprintln!("{}", styled_error_line(msg));
        bail!(msg)
    }
}
