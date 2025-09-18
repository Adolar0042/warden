use anyhow::{Context as _, Result, bail};
use colored::Colorize as _;
use git2::Repository;
use tracing::instrument;

use crate::commands::common::{
    CredentialPair, collect_all_pairs, filter_pairs, labels_credential_host, labels_host_active,
    sort_pairs, styled_error,
};
use crate::config::{Hosts, ProfileConfig};
use crate::load_cfg;
use crate::profile::url::{Patterns, Url as RepoUrl};
use crate::utils::select_index;

#[instrument]
pub fn switch(hostname: Option<&String>, name: Option<&String>, show_all: bool) -> Result<()> {
    let hosts_config = &mut load_cfg!(Hosts)?;
    let profile_config = load_cfg!(ProfileConfig)?;
    if hostname.is_none_or(|h| h.trim().is_empty()) && !show_all {
        let repo = Repository::open_from_env();
        let Ok(repo) = repo else {
            styled_error("Not a git repository!");
            bail!("Not a git repository!");
        };

        let remote = repo.find_remote("origin");
        if let Ok(remote) = remote {
            let remote_url = remote.url().expect("No remote url");
            let url: RepoUrl = match RepoUrl::from_str(remote_url, &profile_config.patterns, None) {
                Ok(u) => u,
                Err(_) => RepoUrl::from_str(remote_url, &Patterns::default(), None)?,
            };
            let host = url.host.to_string();
            if hosts_config.has_host(&host) {
                // only use the repo host if it is known
                return switch_by_host(hosts_config, &host);
            }
        }
    }
    match (hostname, name) {
        (Some(host), Some(credential)) => {
            activate(hosts_config, host, credential).with_context(|| {
                format!("Failed to switch active credential for host '{host}' to '{credential}'")
            })
        },
        (Some(host), None) => {
            switch_by_host(hosts_config, host)
                .with_context(|| format!("Failed to switch active credential for host '{host}'"))
        },
        (None, Some(credential)) => {
            switch_by_credential(hosts_config, credential)
                .with_context(|| format!("Failed to switch to credential '{credential}'"))
        },
        (None, None) => switch_any(hosts_config),
    }
}

fn activate(hosts_config: &mut Hosts, host: &str, credential: &str) -> Result<()> {
    if !hosts_config.has_credential(host, credential) {
        styled_error(format!(
            "No credential named '{credential}' found for host '{host}'",
        ));
        bail!("No credential named '{credential}' found for host '{host}'");
    }
    hosts_config
        .set_active_credential(host, credential)
        .context("Failed to set active credential")?;
    eprintln!(
        "Switched active credential for {} to {}",
        host.bold(),
        credential.bold()
    );
    Ok(())
}

fn switch_by_host(hosts_config: &mut Hosts, host: &str) -> Result<()> {
    let credentials = hosts_config
        .get_credentials(host)
        .with_context(|| format!("Failed to get credentials for host '{host}'"))?
        .to_owned();

    if credentials.is_empty() {
        let msg = format!("No credentials found for host '{host}'");
        styled_error(&msg);
        bail!(msg);
    }

    let target = if credentials.len() == 1 {
        &credentials[0]
    } else if credentials.len() == 2 {
        let active = hosts_config
            .get_active_credential(host)
            .with_context(|| format!("Failed to get active credential for '{host}'"))?;
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
    let mut pairs: Vec<CredentialPair> = collect_all_pairs(hosts_config);
    pairs = filter_pairs(pairs.iter(), None, Some(credential));

    if pairs.is_empty() {
        bail!("No credentials found with name '{credential}'");
    }

    if pairs.len() == 1 {
        return activate(hosts_config, &pairs[0].host, credential);
    }

    sort_pairs(&mut pairs);
    let labels = labels_host_active(&pairs, hosts_config);

    let selection = select_index(
        &labels,
        format!("Select a host to switch to '{credential}'"),
    )?;
    let host = &pairs[selection].host;
    activate(hosts_config, host, credential)
}

fn switch_any(hosts_config: &mut Hosts) -> Result<()> {
    let mut pairs = collect_all_pairs(hosts_config);
    if pairs.is_empty() {
        bail!("No credentials found to switch");
    }
    sort_pairs(&mut pairs);

    // decide which credential to activate
    let target = if pairs.len() == 1 {
        pairs[0].clone()
    } else if pairs.len() == 2 {
        // toggle
        let (first, second) = (&pairs[0], &pairs[1]);
        let active_first = hosts_config
            .get_active_credential(&first.host)
            .is_some_and(|u| u == first.credential);
        if active_first {
            second.clone()
        } else {
            first.clone()
        }
    } else {
        let labels = labels_credential_host(&pairs);
        let selection = select_index(&labels, "Select a credential to switch to")?;
        pairs[selection].clone()
    };

    activate(hosts_config, &target.host, &target.credential)
}
