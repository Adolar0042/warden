use anyhow::{Context as _, Result};
use colored::Colorize as _;
use tracing::{debug, error, info, instrument, warn};

use crate::commands::login::login;
use crate::config::{Hosts, OAuthConfig};
use crate::keyring::get_keyring_token;
use crate::oauth::get_access_token;
use crate::utils::parse_credential_request;

#[instrument(skip(oauth_config, hosts_config))]
pub async fn handle_get(oauth_config: OAuthConfig, hosts_config: &mut Hosts) -> Result<()> {
    info!("Retrieving credentials...");
    let req = parse_credential_request().context("Failed to parse credential request")?;
    debug!("{:#?}", &req);

    // Lookup OAuth provider by host
    let Some(provider) = oauth_config.providers.get(&req.host) else {
        // No config for this host: allow Git to try the next helper.
        warn!("No OAuth provider configuration found for {}", req.host);
        return Ok(());
    };

    if let Some(oauth_only) = oauth_config.oauth_only
        && oauth_only
    {
        debug!("OAuth-only mode is enabled.");
        let token = get_access_token(provider, &oauth_config).await?;

        // Output in Git's expected format
        println!(
            "username={}",
            req.username.unwrap_or_else(|| "oauth".to_string())
        );
        println!("password={token}");
        return Ok(());
    }

    // get request username is not empty and exists in hosts config
    if let Some(username) = &req.username
        && !username.is_empty()
        && hosts_config.has_user(&req.host, username)
    {
        info!("Username was in request and in hosts config.");
        let token = get_keyring_token(username, &req.host)
            .context("Failed to retrieve token from keyring")?;
        println!("username={username}");
        println!("password={}", token.secret());
    }
    // if no username is provided, check if there is an active user for the host
    let mut active_user = hosts_config.get_active_credential(&req.host);
    if active_user.is_none_or(str::is_empty) {
        // if there is no active user, prompt the user to input a username and then
        // perform OAuth
        // assume first use
        login(&oauth_config, hosts_config)
            .await
            .context("Failed to login")?;
        *hosts_config = Hosts::load().context("Failed to reload hosts configuration")?;
        active_user = hosts_config.get_active_credential(&req.host);
        if active_user.is_none() || active_user.is_some_and(str::is_empty) {
            error!("No active user found for host {}", req.host);
            return Err(anyhow::anyhow!(
                "No active user found for host {}. Please login first.",
                req.host
            ));
        }
    }
    let active_user = active_user.unwrap();
    let username = req
        .username
        .clone()
        .unwrap_or_else(|| active_user.to_string());

    if let Ok(token) = get_keyring_token(&username, &req.host) {
        info!("Token found in keyring, returning existing credentials.");
        println!("username={username}");
        println!("password={}", token.secret());
        return Ok(());
    }

    eprintln!(
        "  {} - No credential found for user '{}' on host '{}'.",
        "Error".red().bold(),
        username,
        req.host
    );

    Ok(())
}
