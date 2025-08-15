use std::io::stdout;
use std::process::exit;

use anyhow::{Context as _, Result, bail};
use colored::Colorize as _;
use crossterm::cursor::Show;
use crossterm::execute;
use dialoguer::FuzzySelect;
use tracing::instrument;

use crate::config::Hosts;
use crate::utils::THEME;

#[instrument(skip(hosts_config))]
pub fn switch(
    hosts_config: &mut Hosts,
    hostname: Option<&String>,
    name: Option<&String>,
) -> Result<()> {
    match (hostname, name) {
        (Some(host), Some(credential)) => activate(hosts_config, host, credential),
        (Some(host), None) => switch_by_host(hosts_config, host),
        (None, Some(credential)) => switch_by_credential(hosts_config, credential),
        (None, None) => switch_any(hosts_config),
    }
}

fn activate(hosts_config: &mut Hosts, host: &str, credential: &str) -> Result<()> {
    hosts_config
        .set_active_credential(host, credential)
        .context("Failed to set active credential.")?;
    eprintln!(
        "Switched active credential for {} to {}",
        host.bold(),
        credential.bold()
    );
    Ok(())
}

fn select_index(items: &[String], prompt: impl Into<String>) -> Result<usize> {
    let _ = ctrlc::set_handler(|| {
        let _ = execute!(stdout(), Show);
        exit(130);
    });
    FuzzySelect::with_theme(&*THEME)
        .items(items)
        .with_prompt(prompt)
        .default(0)
        .interact()
        .context("Failed to select")
}

fn switch_by_host(hosts_config: &mut Hosts, host: &str) -> Result<()> {
    let credentials = hosts_config
        .get_users(host)
        .context(format!("Failed to get credentials for host '{host}'"))?
        .to_owned();

    if credentials.is_empty() {
        eprintln!(
            "  {} - No credentials found for host '{host}'",
            "Error".red().bold()
        );
        bail!("No credentials found for host '{host}'");
    }

    let target = if credentials.len() == 1 {
        &credentials[0]
    } else if credentials.len() == 2 {
        let active = hosts_config
            .get_active_credential(host)
            .context(format!("Failed to get active credential for '{host}'"))?;
        if credentials[0] == active {
            &credentials[1]
        } else {
            &credentials[0]
        }
    } else {
        let selection = select_index(&credentials, format!("Select a credential for {host}"))?;
        &credentials[selection]
    };

    activate(hosts_config, host, target)
}

fn switch_by_credential(hosts_config: &mut Hosts, credential: &str) -> Result<()> {
    let mut pairs: Vec<(String, String)> = hosts_config
        .hosts()
        .filter(|(_, cfg)| cfg.users.iter().any(|n| n == credential))
        .map(|(h, c)| (h.to_string(), c.active.clone()))
        .collect();

    if pairs.is_empty() {
        bail!("No credentials found for '{credential}'.");
    }

    if pairs.len() == 1 {
        let (host, _) = pairs.pop().unwrap();
        return activate(hosts_config, &host, credential);
    }

    let labels: Vec<String> = pairs
        .iter()
        .map(|(h, active)| format!("{h} ({active})"))
        .collect();

    let selection = select_index(
        &labels,
        format!("Select a host to switch to '{credential}'"),
    )?;
    let (host, _) = &pairs[selection];
    activate(hosts_config, host, credential)
}

fn switch_any(hosts_config: &mut Hosts) -> Result<()> {
    let mut pairs: Vec<(String, String)> = hosts_config
        .hosts()
        .flat_map(|(h, cfg)| cfg.users.iter().map(move |n| (h.to_string(), n.clone())))
        .collect();

    if pairs.is_empty() {
        bail!("No credentials found to switch.");
    }

    pairs.sort_by(|(ha, na), (hb, nb)| ha.cmp(hb).then_with(|| na.cmp(nb)));

    let (host, credential) = if pairs.len() == 1 {
        pairs[0].clone()
    } else if pairs.len() == 2 {
        let (h1, c1) = &pairs[0];
        let (h2, c2) = &pairs[1];
        let active = hosts_config
            .get_active_credential(h1)
            .or_else(|| hosts_config.get_active_credential(h2))
            .context("Failed to get active credential")?;
        if active == c1 {
            (h2.clone(), c2.clone())
        } else {
            (h1.clone(), c1.clone())
        }
    } else {
        let labels: Vec<String> = pairs.iter().map(|(h, n)| format!("{n} ({h})")).collect();
        let selection = select_index(&labels, "Select a credential to switch to")?;
        pairs[selection].clone()
    };

    activate(hosts_config, &host, &credential)
}
