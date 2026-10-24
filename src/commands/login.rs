use std::io::stderr;
use std::process::exit;

use anyhow::{Context as _, Result, bail};
use crossterm::cursor::Show;
use crossterm::execute;
use crossterm::style::Stylize as _;
use dialoguer::{Confirm, Input};
use tracing::instrument;

use crate::config::{Hosts, OAuthConfig};
use crate::keyring::store_keyring_token;
use crate::load_cfg;
use crate::oauth::get_access_token;
use crate::theme::InputTheme;
use crate::utils::{config_dir, select_index};

#[instrument]
pub async fn login(force_device: bool) -> Result<()> {
    login_for(force_device, None, None).await
}

#[instrument]
pub async fn login_for(
    force_device: bool,
    requested_credential: Option<&str>,
    requested_host: Option<&str>,
) -> Result<()> {
    let oauth_config = load_cfg!(OAuthConfig)?;
    let mut hosts_config = load_cfg!(Hosts)?;
    let _ = ctrlc::set_handler(|| {
        let _ = execute!(stderr(), Show);
        exit(130);
    });
    let credential_name = if let Some(credential) = requested_credential {
        let credential = credential.trim().to_string();
        eprintln!("Logging in for credential '{credential}'...");
        credential
    } else {
        Input::with_theme(&InputTheme::default())
            .with_prompt("Credential Name")
            .default("oauth".to_string())
            .interact_text()
            .context("Failed to read credential name")?
            .trim()
            .to_string()
    };
    if credential_name.trim().is_empty() {
        bail!("Credential name cannot be empty!");
    }
    let mut providers = oauth_config.providers.keys().collect::<Vec<_>>();
    if providers.is_empty() {
        bail!(
            "No OAuth providers configured! Please add at least one provider in {}.",
            config_dir()
                .context("Failed to get config directory")?
                .join("oauth.toml")
                .display()
        );
    }
    providers.sort();
    let provider = if let Some(host) = requested_host {
        oauth_config
            .providers
            .get_key_value(host)
            .map(|(provider, _)| provider)
            .with_context(|| format!("No OAuth provider configuration found for {host}"))?
    } else {
        let selection = select_index(&providers, "Host").context("Failed to select host")?;
        providers[selection]
    };

    // if host already has a credential under that name, ask for confirmation
    if hosts_config.has_credential(provider, &credential_name) {
        let _ = ctrlc::set_handler(|| {
            let _ = execute!(stderr(), Show);
            exit(130);
        });
        eprintln!(
            "{}",
            format!(
                "A credential with the name '{credential_name}' already exists for host \
                 '{provider}'."
            )
            .bold()
        );
        let confirm = Confirm::with_theme(&InputTheme::default())
            .with_prompt("Do you want to overwrite it?")
            .default(false)
            .interact_opt()
            .context("Failed to confirm overwrite")?;
        if confirm.is_none_or(|b| !b) {
            exit(1);
        }
    }

    let token = get_access_token(&oauth_config, provider, force_device)
        .await
        .context("Failed to get access token")?;

    store_keyring_token(&credential_name, provider, &token)
        .context("Failed to store token in keyring")?;
    hosts_config
        .add_credential(provider, &credential_name)
        .context("Failed to add credential to hosts state")?;
    Ok(())
}
