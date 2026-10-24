use anyhow::{Context as _, Result, bail};
use chrono::{DateTime, Utc};
use tracing::{debug, error, info, instrument, warn};

use crate::commands::common::styled_error;
use crate::commands::login::login_for;
use crate::commands::{print_token, print_token_checked};
use crate::config::{Hosts, OAuthConfig, ProviderConfig};
use crate::keyring::{Token, get_keyring_token};
use crate::load_cfg;
use crate::oauth::get_access_token;
use crate::utils::{CredentialRequest, parse_credential_request};

#[instrument(skip(req, provider))]
async fn maybe_print_with_refresh_token(
    req: &CredentialRequest,
    provider: &ProviderConfig,
) -> Result<bool> {
    // A refresh token was provided, and no password (access token) was provided
    // If so, use the refresh token to get a new access token and print it
    let Some(refresh_token) = req
        .oauth_refresh_token
        .as_ref()
        .filter(|_| req.password.is_none())
    else {
        return Ok(false);
    };

    info!("Using provided refresh token to get new access token");
    let mut token = Token::new(
        String::new(),
        Some(refresh_token.clone()),
        DateTime::<Utc>::from_timestamp(0, 0),
    )
    .context("Failed to protect provided refresh token in memory")?;
    print_token_checked(
        &mut token,
        req.username.as_deref().unwrap_or("oauth"),
        provider,
    )
    .await
    .context("Failed to print token")?;
    Ok(true)
}

async fn resolve_username(
    requested_username: Option<&str>,
    host: &str,
    hosts_config: &mut Hosts,
    force_device: bool,
) -> Result<String> {
    if let Some(username) = requested_username.filter(|username| !username.trim().is_empty()) {
        if !hosts_config.has_credential(host, username) {
            eprintln!(
                "No credential found for user '{username}' on host '{host}'.\nPlease login first."
            );
            login_for(force_device, Some(username), Some(host))
                .await
                .context("Failed to login")?;
            *hosts_config = load_cfg!(Hosts)?;
            if !hosts_config.has_credential(host, username) {
                bail!("Login did not create credential '{username}' for host '{host}'");
            }
        }

        return Ok(username.to_owned());
    }

    if hosts_config
        .get_active_credential(host)
        .is_none_or(|credential| credential.trim().is_empty())
    {
        eprintln!("No active credential found for host {host}.\nPlease login first.");
        login_for(force_device, None, Some(host))
            .await
            .context("Failed to login")?;
        *hosts_config = load_cfg!(Hosts)?;
    }

    let Some(username) = hosts_config
        .get_active_credential(host)
        .filter(|credential| !credential.trim().is_empty())
    else {
        error!("No active credential found for host {host}");
        bail!("No active credential found for host {host}. Please login first.");
    };

    Ok(username.to_owned())
}

#[instrument]
pub async fn handle_get(force_device: bool) -> Result<()> {
    info!("Retrieving credentials...");
    let req = parse_credential_request().context("Failed to parse credential request")?;
    debug!("{:#?}", &req);

    let oauth_config = load_cfg!(OAuthConfig)?;
    let mut hosts_config = load_cfg!(Hosts)?;

    // Lookup OAuth provider by host
    let Some(provider) = oauth_config.providers.get(&req.host) else {
        // No config for this host, allow Git to try the next helper
        warn!("No OAuth provider configuration found for {}", req.host);
        return Ok(());
    };

    if oauth_config.oauth_only.unwrap_or(false) {
        debug!("OAuth-only mode is enabled");

        if maybe_print_with_refresh_token(&req, provider).await? {
            return Ok(());
        }

        let token = get_access_token(&oauth_config, &req.host, force_device).await?;
        print_token(&token, &req.username.unwrap_or_else(|| "oauth".to_string()));
        return Ok(());
    }

    let username = resolve_username(
        req.username.as_deref(),
        &req.host,
        &mut hosts_config,
        force_device,
    )
    .await?;

    let mut token = match get_keyring_token(&username, &req.host) {
        Ok(token) => token,
        Err(error)
            if matches!(
                error.downcast_ref::<keyring_core::Error>(),
                Some(keyring_core::Error::NoEntry)
            ) =>
        {
            warn!(
                "No credential found for '{username}' on host '{}'.",
                req.host
            );
            styled_error(format!(
                "No credential found for user '{username}' on host '{}'.",
                req.host
            ));
            return Ok(());
        },
        Err(error) => return Err(error).context("Failed to retrieve token from keyring"),
    };

    info!(
        "Using cached credential for '{username}' on '{}'.",
        req.host
    );
    print_token_checked(&mut token, &username, provider)
        .await
        .context("Failed to output token")?;

    Ok(())
}
