use anyhow::{Context as _, Result, bail};
use chrono::{DateTime, Utc};
use tracing::{debug, error, info, instrument, warn};

use crate::commands::common::styled_error_line;
use crate::commands::login::login;
use crate::commands::{print_token, print_token_checked};
use crate::config::{Hosts, OAuthConfig, ProviderConfig};
use crate::keyring::{Token, get_keyring_token};
use crate::oauth::{device_code, get_access_token};
use crate::utils::{CredentialRequest, parse_credential_request};

#[instrument(skip(req, provider))]
async fn maybe_print_with_refresh_token(
    req: &CredentialRequest,
    provider: &ProviderConfig,
) -> Result<bool> {
    if let Some(refresh_token) = req.oauth_refresh_token.as_ref()
        && req.password.is_none()
    {
        info!("Using provided refresh token to get access token.");
        let mut token = Token::new(
            req.password.clone().unwrap_or_default(),
            Some(refresh_token.clone()),
            DateTime::<Utc>::from_timestamp(0, 0),
        );
        print_token_checked(
            &mut token,
            &req.username.clone().unwrap_or_else(|| "oauth".to_string()),
            provider,
        )
        .await
        .context("Failed to print token")?;
        return Ok(true);
    }
    Ok(false)
}

#[instrument(skip(oauth_config, hosts_config))]
pub async fn handle_get(
    oauth_config: OAuthConfig,
    hosts_config: &mut Hosts,
    force_device: bool,
) -> Result<()> {
    info!("Retrieving credentials...");
    let req = parse_credential_request().context("Failed to parse credential request")?;
    debug!("{:#?}", &req);

    // Lookup OAuth provider by host
    let Some(provider) = oauth_config.providers.get(&req.host) else {
        // No config for this host: allow Git to try the next helper.
        warn!("No OAuth provider configuration found for {}", req.host);
        return Ok(());
    };

    if force_device {
        if provider.device_auth_url.is_none() {
            error!("Device code flow is not supported for this provider.");
            bail!("Device code flow is not supported for this provider.");
        }
        if maybe_print_with_refresh_token(&req, provider).await? {
            return Ok(());
        }
        let token = device_code::exchange_device_code(provider, &oauth_config)
            .await
            .context("Failed to authenticate with device flow.")?;
        print_token(&token, &req.username.unwrap_or_else(|| "oauth".to_string()));
        return Ok(());
    }

    if oauth_config.oauth_only.unwrap_or(false) {
        debug!("OAuth-only mode is enabled.");
        if maybe_print_with_refresh_token(&req, provider).await? {
            return Ok(());
        }
        let token = get_access_token(provider, &oauth_config, force_device).await?;
        print_token(&token, &req.username.unwrap_or_else(|| "oauth".to_string()));
        return Ok(());
    }

    // if a username was provided, and we know it, return its credential
    if let Some(credential) = &req.username
        && !credential.is_empty()
        && hosts_config.has_credential(&req.host, credential)
    {
        info!("Username was in request and in hosts config.");
        let mut token = get_keyring_token(credential, &req.host)
            .context("Failed to retrieve token from keyring")?;
        print_token_checked(&mut token, credential, provider)
            .await
            .context("Failed to print token")?;
        return Ok(());
    }
    // if no username is provided, check if there is an active user for the host
    let mut active_credential = hosts_config.get_active_credential(&req.host);
    if active_credential.is_none_or(str::is_empty) {
        // if there is no active credential, prompt the user to input a credential name
        // and then perform first use login flow
        eprintln!(
            " No active credential found for host {}.\n Please login first.",
            req.host
        );
        login(&oauth_config, hosts_config, force_device)
            .await
            .context("Failed to login")?;
        *hosts_config = Hosts::load().context("Failed to reload hosts configuration")?;
        active_credential = hosts_config.get_active_credential(&req.host);
        if active_credential.is_none() || active_credential.is_some_and(str::is_empty) {
            error!("No active credential found for host {}", req.host);
            bail!(
                "No active credential found for host {}. Please login first.",
                req.host
            );
        }
    }
    let active_credential = active_credential.unwrap();
    let username = req.username.as_deref().unwrap_or(active_credential);

    if let Ok(mut token) = get_keyring_token(username, &req.host) {
        info!(
            "Using cached credential for '{username}' on '{}'.",
            req.host
        );
        print_token_checked(&mut token, username, provider)
            .await
            .context("Failed to print token")?;
        return Ok(());
    }

    warn!("No credential found for '{username}' on '{}'.", req.host);
    eprintln!(
        "{}",
        styled_error_line(format!(
            "No credential found for user '{username}' on host '{}'.",
            req.host
        ))
    );

    Ok(())
}
