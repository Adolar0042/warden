use anyhow::{Context as _, Result};
use tracing::{info, instrument, warn};

use crate::config::OAuthConfig;
use crate::keyring::store_keyring_token;
use crate::utils::parse_credential_request;

#[instrument(skip(oauth_config))]
pub async fn handle_store(oauth_config: OAuthConfig) -> Result<()> {
    if oauth_config.oauth_only.is_some_and(|x| x) {
        return Ok(());
    }
    info!("Storing credentials...");
    let req = parse_credential_request().context("Failed to parse credential request")?;
    if let Some(username) = &req.username
        && let Some(password) = &req.password
    {
        store_keyring_token(username, &req.host, password)
            .context("Failed to store token in keyring")?;
        Ok(())
    } else {
        warn!("No username or password provided in request; nothing to store.");
        Err(anyhow::anyhow!(
            "No username or password provided in request; nothing to store."
        ))
    }
}
