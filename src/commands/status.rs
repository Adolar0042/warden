use anyhow::{Result, bail};
use colored::Colorize as _;
use tracing::instrument;

use crate::commands::common::styled_error_line;
use crate::config::Hosts;
use crate::keyring::get_keyring_token;

#[instrument(skip(hosts_config))]
pub fn status(hosts_config: &Hosts) -> Result<()> {
    if hosts_config.is_empty() {
        eprintln!(
            "{}",
            styled_error_line(format!(
                "No credentials found. Add credentials by running {}.",
                format!("{} login", env!("CARGO_PKG_NAME")).blue()
            ))
        );
        bail!("No credentials found");
    }

    for (host, config) in hosts_config.iter_sorted() {
        if config.credentials.is_empty() {
            eprintln!("{}: No credentials found.", host.bold());
            continue;
        }

        let active_credential = &config.active;
        if active_credential.is_empty() {
            eprintln!("{}: no active credential", host.bold());
        } else {
            let token = get_keyring_token(active_credential, host);
            if let Ok(token) = token {
                eprintln!("{}: {active_credential} ({token})", host.bold());
            } else {
                eprintln!("{}: {}", host.bold(), active_credential.red());
            }
        }

        let mut credentials: Vec<&String> = config
            .credentials
            .iter()
            .filter(|u| *u != active_credential)
            .collect();
        credentials.sort();

        for credential_name in credentials {
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
