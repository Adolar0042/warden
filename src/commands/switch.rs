use anyhow::{Context as _, Result, anyhow};
use colored::Colorize as _;
use tracing::instrument;

use crate::config::Hosts;
use crate::utils::THEME;

#[instrument(skip(hosts_config))]
#[expect(
    clippy::too_many_lines,
    reason = "This one big function handles multiple cases"
)]
pub fn switch(
    hosts_config: &mut Hosts,
    hostname: Option<&String>,
    name: Option<&String>,
) -> Result<()> {
    if let Some(host) = hostname {
        if let Some(name) = name {
            hosts_config.set_active_credential(host, name)?;
            eprintln!("Switched active credential for {host} to {}", name.bold());
        } else {
            let target_name = {
                let credentials = hosts_config
                    .get_users(host)
                    .context(format!("Failed to get credentials for host '{host}'"))?;

                if credentials.is_empty() {
                    eprintln!("No credentials found for host '{host}'");
                    return Err(anyhow!("No credentials found for host '{host}'"));
                }

                if credentials.len() == 1 {
                    credentials[0].clone()
                } else if credentials.len() == 2 {
                    // make the non-active user the active user
                    let active_user = hosts_config
                        .get_active_credential(host)
                        .context(format!("Failed to get active credential for {host}"))?;
                    if credentials[0] == active_user {
                        credentials[1].clone()
                    } else {
                        credentials[0].clone()
                    }
                } else {
                    let selection = dialoguer::FuzzySelect::with_theme(&*THEME)
                        .items(credentials)
                        .with_prompt(format!("Select a credential for {host}"))
                        .default(0)
                        .interact()
                        .context("Failed to select credential")?;
                    credentials[selection].clone()
                }
            };

            hosts_config
                .set_active_credential(host, &target_name)
                .context("Failed to set active credential.")?;
            eprintln!(
                "Switched active credential for {host} to {}",
                target_name.bold()
            );
        }
    } else if let Some(credential_name) = name {
        // No host provided, find all hosts that have this credential
        // This is not really how it is intended to be used, but it is
        // supported for convenience.
        let mut hosts_with_credential_name: Vec<String> = hosts_config
            .hosts()
            .filter(|(_, cfg)| cfg.users.iter().any(|n| n == credential_name))
            .map(|(h, _)| h.to_string())
            .collect();

        if hosts_with_credential_name.is_empty() {
            eprintln!(
                "  {} - No credentials found for '{credential_name}'.",
                "Error".red().bold()
            );
            return Err(anyhow!("No credentials found for '{credential_name}'."));
        }

        if hosts_with_credential_name.len() == 1 {
            let host = hosts_with_credential_name.remove(0);
            hosts_config
                .set_active_credential(&host, credential_name)
                .context("Failed to set active credential.")?;
            eprintln!(
                "Switched active credential for {host} to {}",
                credential_name.bold()
            );
        } else {
            let selection = dialoguer::FuzzySelect::with_theme(&*THEME)
                .items(&hosts_with_credential_name)
                .with_prompt(format!("Select a host to switch for '{credential_name}'"))
                .default(0)
                .interact()
                .context("Failed to select host")?;
            let host = &hosts_with_credential_name[selection];

            hosts_config
                .set_active_credential(host, credential_name)
                .context("Failed to set active credential.")?;
            eprintln!(
                "Switched active credential for {host} to {}",
                credential_name.bold()
            );
        }
    } else {
        // Neither host nor credential provided, let the user select from all
        // credentials
        let mut host_credential_pairs: Vec<(String, String)> = hosts_config
            .hosts()
            .flat_map(|(host, cfg)| {
                cfg.users
                    .iter()
                    .map(move |credential_name| (host.to_string(), credential_name.clone()))
            })
            .collect();

        // sort by host then by credential name
        host_credential_pairs.sort_by(|(ha, na), (hb, nb)| ha.cmp(hb).then_with(|| na.cmp(nb)));

        if host_credential_pairs.is_empty() {
            eprintln!(
                "  {} - No credentials found to switch.",
                "Error".red().bold()
            );
            return Err(anyhow!("No credentials found to switch."));
        }

        let (host, credential_name) = if host_credential_pairs.len() == 1 {
            host_credential_pairs[0].clone()
        } else if host_credential_pairs.len() == 2 {
            // make the non-active user the active user
            let (host1, credential1) = &host_credential_pairs[0];
            let (host2, credential2) = &host_credential_pairs[1];
            let active_user = hosts_config
                .get_active_credential(host1)
                .or_else(|| hosts_config.get_active_credential(host2))
                .context("Failed to get active credential")?;

            if credential1 == active_user {
                (host2.clone(), credential2.clone())
            } else {
                (host1.clone(), credential1.clone())
            }
        } else {
            let credentials: Vec<String> = host_credential_pairs
                .iter()
                .map(|(host, credential_name)| format!("{credential_name} ({host})"))
                .collect();

            let selection = dialoguer::FuzzySelect::with_theme(&*THEME)
                .items(&credentials)
                .with_prompt("Select a credential to switch")
                .default(0)
                .interact()
                .context("Failed to select credential")?;
            host_credential_pairs[selection].clone()
        };

        hosts_config
            .set_active_credential(&host, &credential_name)
            .context("Failed to set active credential.")?;
        eprintln!(
            "Switched active credential for {host} to {}",
            credential_name.bold()
        );
    }
    Ok(())
}
