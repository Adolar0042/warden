use std::io::stderr;
use std::process::exit;

use anyhow::{Context as _, Result, bail};
use crossterm::cursor::Show;
use crossterm::execute;
use dialoguer::Confirm;

use crate::commands::common::{
    CredentialPair, collect_all_pairs, filter_pairs, sort_pairs, styled_error_line,
};
use crate::config::{Hosts, OAuthConfig};
use crate::keyring::{get_keyring_token, store_keyring_token};
use crate::oauth::{get_access_token, refresh_access_token};
use crate::theme::InputTheme;
use crate::utils::select_index;

pub async fn refresh(
    oauth_config: &OAuthConfig,
    hosts_config: &Hosts,
    host: Option<&str>,
    name: Option<&str>,
    force_device: bool,
) -> Result<()> {
    let mut pairs = collect_all_pairs(hosts_config);
    if pairs.is_empty() {
        eprintln!("{}", styled_error_line("No credentials found to refresh."));
        bail!("No credentials found to refresh.");
    }
    sort_pairs(&mut pairs);

    let filtered = filter_pairs(&pairs, host, name);

    if filtered.is_empty() {
        match (host, name) {
            (Some(h), Some(n)) => {
                let msg = format!("No credentials found for '{n}' on {h}.");
                eprintln!("{}", styled_error_line(&msg));
                bail!(msg);
            },
            (Some(h), None) => {
                let msg = format!("No credentials found for {h}.");
                eprintln!("{}", styled_error_line(&msg));
                bail!(msg);
            },
            (None, Some(n)) => {
                let msg = format!("No credentials found for '{n}'.");
                eprintln!("{}", styled_error_line(&msg));
                bail!(msg);
            },
            (None, None) => {
                let msg = "No credentials found to refresh.".to_string();
                eprintln!("{}", styled_error_line(&msg));
                bail!(msg);
            },
        }
    }

    let target = if filtered.len() == 1 {
        filtered[0].clone()
    } else {
        let labels: Vec<String> = filtered
            .iter()
            .map(|p| {
                match get_keyring_token(&p.credential, &p.host) {
                    Ok(_) => format!("{} ({})", p.credential, p.host),
                    Err(_) => format!("{} ({}) - not in keyring", p.credential, p.host),
                }
            })
            .collect();
        let selection = select_index(&labels, "Select a credential to refresh")?;
        filtered[selection].clone()
    };

    refresh_one(oauth_config, &target, force_device).await
}

/// Refresh a single credential, use refresh token if present and approved,
/// otherwise run a full OAuth flow.
async fn refresh_one(
    oauth_config: &OAuthConfig,
    pair: &CredentialPair,
    force_device: bool,
) -> Result<()> {
    let provider = oauth_config
        .providers
        .get(&pair.host)
        .context("Provider not found")?;

    if let Ok(token) = get_keyring_token(&pair.credential, &pair.host)
        && token.refresh_token().is_some()
    {
        let _ = ctrlc::set_handler(|| {
            let _ = execute!(stderr(), Show);
            exit(130);
        });
        let use_refresh = Confirm::with_theme(&InputTheme::default())
            .with_prompt("A refresh token is available. Use it?")
            .default(true)
            .interact()
            .context("Failed to confirm refresh token usage")?;
        if use_refresh {
            let token = refresh_access_token(provider, &token)
                .await
                .context("Failed to refresh access token")?;
            store_keyring_token(pair.credential.as_str(), &pair.host, &token)
                .context("Failed to store refreshed token in keyring")?;
            return Ok(());
        }
    }

    let token = get_access_token(provider, oauth_config, force_device)
        .await
        .context("Failed to get access token")?;
    store_keyring_token(pair.credential.as_str(), &pair.host, &token)
        .context("Failed to store token in keyring")?;
    Ok(())
}
