use anyhow::{Context as _, Result, anyhow};
use colored::Colorize as _;

use crate::config::{Hosts, OAuthConfig};
use crate::keyring::{get_keyring_token, store_keyring_token};
use crate::oauth::get_access_token;
use crate::utils::THEME;

pub async fn refresh(
    oauth_config: &OAuthConfig,
    hosts_config: &Hosts,
    host: Option<&String>,
    name: Option<&String>,
) -> Result<()> {
    let mut host_user_pairs: Vec<(String, String)> = hosts_config
        .hosts()
        .flat_map(|(host, cfg)| {
            cfg.users
                .iter()
                .map(move |username| (host.to_string(), username.clone()))
        })
        .collect();
    // sort by host then by user
    host_user_pairs.sort_by(|(ha, na), (hb, nb)| ha.cmp(hb).then_with(|| na.cmp(nb)));
    if host_user_pairs.is_empty() {
        eprintln!(
            "  {} - No credentials found to refresh.",
            "Error".red().bold()
        );
        return Err(anyhow!("No credentials found to refresh."));
    }
    // Apply provided filters
    let mut filtered_pairs = host_user_pairs.clone();
    if let Some(h) = host {
        filtered_pairs.retain(|(host, _)| host == h);
    }
    if let Some(n) = name {
        filtered_pairs.retain(|(_, username)| username == n);
    }
    if filtered_pairs.is_empty() {
        match (&host, &name) {
            (Some(h), Some(n)) => {
                eprintln!(
                    "  {} - No credentials found for '{n}' on {h}.",
                    "Error".red().bold()
                );
                return Err(anyhow!("No credentials found for '{n}' on {h}."));
            },
            (Some(h), None) => {
                eprintln!("  {} - No credentials found for {h}.", "Error".red().bold());
                return Err(anyhow!("No credentials found for {h}."));
            },
            (None, Some(n)) => {
                eprintln!(
                    "  {} - No credentials found for '{n}'.",
                    "Error".red().bold()
                );
                return Err(anyhow!("No credentials found for '{n}'."));
            },
            (None, None) => {
                eprintln!(
                    "  {} - No credentials found to refresh.",
                    "Error".red().bold()
                );
                return Err(anyhow!("No credentials found to refresh."));
            },
        }
    }
    // If we have a single pair, refresh it
    if filtered_pairs.len() == 1 {
        let (host, username) = filtered_pairs.remove(0);
        let provider = oauth_config
            .providers
            .get(&host)
            .context("Provider not found")?;
        let token = get_access_token(provider, oauth_config)
            .await
            .context("Failed to get access token")?;
        store_keyring_token(username.as_str(), &host, token.as_str())
            .context("Failed to store token in keyring")?;
    } else {
        // If we have multiple pairs, let the user select one
        let credentials: Vec<String> = filtered_pairs
            .iter()
            .map(|(host, credential_name)| -> String {
                let token = get_keyring_token(credential_name, host);
                match token {
                    Ok(_) => {
                        format!("{credential_name} ({host})")
                    },
                    Err(_) => {
                        format!("{credential_name} ({host}) - not found in keyring")
                    },
                }
            })
            .collect();
        let selection = dialoguer::FuzzySelect::with_theme(&*THEME)
            .items(&credentials)
            .with_prompt("Select a credential to refresh")
            .default(0)
            .interact()
            .context("Failed to select credential")?;
        let (host, username) = &filtered_pairs[selection];
        let provider = oauth_config
            .providers
            .get(host)
            .context("Provider not found")?;
        let token = get_access_token(provider, oauth_config)
            .await
            .context("Failed to get access token")?;
        store_keyring_token(username.as_str(), host, token.as_str())
            .context("Failed to store token in keyring")?;
    }
    Ok(())
}
