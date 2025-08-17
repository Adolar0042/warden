use std::io::stderr;
use std::process::exit;

use anyhow::{Context as _, Result, bail};
use crossterm::cursor::Show;
use crossterm::execute;
use dialoguer::Input;
use tracing::instrument;

use crate::config::{Hosts, OAuthConfig};
use crate::keyring::store_keyring_token;
use crate::oauth::get_access_token;
use crate::utils::{THEME, config_dir, select_index};

#[instrument(skip(oauth_config, hosts_config))]
pub async fn login(
    oauth_config: &OAuthConfig,
    hosts_config: &mut Hosts,
    force_device: bool,
) -> Result<()> {
    let _ = ctrlc::set_handler(|| {
        let _ = execute!(stderr(), Show);
        exit(130);
    });
    let credential_name: String = Input::with_theme(&*THEME)
        .with_prompt("Credential Name")
        .default("oauth".to_string())
        .interact_text()
        .context("Failed to read credential name")?;
    let credential_name = credential_name.trim();
    if credential_name.is_empty() {
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
    let selection = select_index(&providers, "Host").context("Failed to select host")?;

    // if host already has a credential under that name, ask for confirmation
    if hosts_config.has_credential(providers[selection], credential_name) {
        let _ = ctrlc::set_handler(|| {
            let _ = execute!(stderr(), Show);
            exit(130);
        });
        let confirm = dialoguer::Confirm::with_theme(&*THEME)
            .with_prompt(format!(
                "A credential with the name '{}' already exists for host '{}'. Do you want to \
                 overwrite it?",
                credential_name, providers[selection]
            ))
            .default(false)
            .wait_for_newline(true)
            .interact()
            .context("Failed to confirm overwrite")?;
        if !confirm {
            exit(1);
        }
    }

    let provider = oauth_config
        .providers
        .get(providers[selection])
        .context("Provider not found")?;
    let token = get_access_token(provider, oauth_config, force_device)
        .await
        .context("Failed to get access token")?;

    store_keyring_token(credential_name, providers[selection], &token)
        .context("Failed to store token in keyring")?;
    hosts_config
        .add_credential(providers[selection], credential_name)
        .context("Failed to add credential to hosts state")?;
    Ok(())
}
