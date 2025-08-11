use anyhow::{Result, anyhow};
use colored::Colorize as _;
use tracing::instrument;

use crate::config::Hosts;
use crate::keyring::get_keyring_token;

#[instrument(skip(hosts_config))]
pub fn status(hosts_config: &Hosts) -> Result<()> {
    let mut hosts = hosts_config.hosts().peekable();
    if hosts.peek().is_none() {
        eprintln!("  {} - No credentials found.", "Error".red().bold());
        return Err(anyhow!("No credentials found."));
    }

    for (host, config) in hosts {
        if config.users.is_empty() {
            eprintln!("{}: no users configured", host.bold());
            continue;
        }

        let active_credential = &config.active;
        if active_credential.is_empty() {
            eprintln!("{}: no active user", host.bold());
        } else {
            let token = get_keyring_token(active_credential, host);
            if let Ok(token) = token {
                eprintln!("{}: {active_credential} ({token})", host.bold());
            } else {
                eprintln!("{}: {}", host.bold(), active_credential.red());
            }
        }

        for credential_name in &config.users {
            if credential_name == active_credential {
                continue;
            }
            let token = get_keyring_token(credential_name, host);
            if let Ok(token) = token {
                eprintln!("  - {credential_name} ({token})");
            } else {
                eprintln!("  - {}", credential_name.red());
            }
        }
    }
    Ok(())
}
