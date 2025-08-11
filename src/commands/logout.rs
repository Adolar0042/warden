use anyhow::{Context as _, Result};
use colored::Colorize as _;
use dialoguer::FuzzySelect;
use tracing::instrument;

use crate::config::Hosts;
use crate::utils::THEME;

#[instrument(skip(hosts_config))]
pub fn logout(
    hosts_config: &mut Hosts,
    hostname: Option<&String>,
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
            "  {} - No credentials found to logout.",
            "Error".red().bold()
        );
        return Err(anyhow::anyhow!("No credentials found to logout."));
    }

    // Apply provided filters
    let mut filtered_pairs = host_user_pairs.clone();
    if let Some(h) = hostname {
        filtered_pairs.retain(|(host, _)| host == h);
    }
    if let Some(n) = name {
        filtered_pairs.retain(|(_, username)| username == n);
    }

    if filtered_pairs.is_empty() {
        match (&hostname, &name) {
            (Some(h), Some(n)) => {
                eprintln!(
                    "  {} - No credentials found for '{n}' on {h}.",
                    "Error".red().bold()
                );
                Err(anyhow::anyhow!("No credentials found for '{n}' on {h}."))
            },
            (Some(h), None) => {
                eprintln!("  {} - No credentials found for {h}.", "Error".red().bold());
                Err(anyhow::anyhow!("No credentials found for {h}."))
            },
            (None, Some(n)) => {
                eprintln!(
                    "  {} - No credentials found for '{n}'.",
                    "Error".red().bold()
                );
                Err(anyhow::anyhow!("No credentials found for '{n}'."))
            },
            (None, None) => {
                // This branch shouldn't be reachable because we already checked the unfiltered
                // list.
                eprintln!(
                    "  {} - No credentials found to logout.",
                    "Error".red().bold()
                );
                Err(anyhow::anyhow!("No credentials found to logout."))
            },
        }?;
    }

    // If we have an exact match (both provided) or only one candidate remains, no
    // need to prompt
    let (host, credential_name) =
        if (hostname.is_some() && name.is_some()) || filtered_pairs.len() == 1 {
            filtered_pairs[0].clone()
        } else {
            // Build credentials list for the selector from filtered pairs
            let credentials: Vec<String> = filtered_pairs
                .iter()
                .map(|(host, credential_name)| format!("{credential_name} ({host})"))
                .collect();

            let prompt = match (&hostname, &name) {
                (Some(h), None) => format!("Select a credential to logout on {h}"),
                (None, Some(n)) => format!("Select a host to logout for '{n}'"),
                _ => "Select a credential to logout".to_string(),
            };

            let selection = FuzzySelect::with_theme(&*THEME)
                .items(&credentials)
                .with_prompt(prompt)
                .default(0)
                .interact()
                .context("Failed to select host")?;

            filtered_pairs[selection].clone()
        };

    if !hosts_config
        .remove_user(&host, &credential_name)
        .context("Failed to remove credential from hosts configuration")?
    {
        eprintln!(
            "  {} - Failed to remove credential {credential_name} for host {host} from hosts \
             configuration.",
            "Error".red().bold()
        );
        return Err(anyhow::anyhow!(
            "Failed to remove credential {credential_name} for host {host} from hosts \
             configuration."
        ));
    }
    eprintln!(
        "Successfully logged out {} {}",
        credential_name,
        format!("({host})").dimmed()
    );
    Ok(())
}
