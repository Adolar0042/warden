use anyhow::{Context as _, Result, bail};
use tracing::{instrument, warn};

use crate::commands::common::styled_error_line;
use crate::config::OAuthConfig;
use crate::keyring::erase_keyring_token;
use crate::utils::parse_credential_request;

#[instrument(skip(oauth_config))]
pub async fn handle_erase(oauth_config: OAuthConfig) -> Result<()> {
    if oauth_config.oauth_only.is_some_and(|x| x) {
        return Ok(());
    }
    tracing::info!("Erasing credentials...");
    let req = parse_credential_request().context("Failed to parse credential request")?;
    if let Some(username) = &req.username {
        erase_keyring_token(username, &req.host)
            .context("Failed to erase credential from keyring")?;
        Ok(())
    } else {
        let msg = "No username provided in request; nothing to erase.";
        warn!("{msg}");
        eprintln!("{}", styled_error_line(msg));
        bail!(msg)
    }
}
