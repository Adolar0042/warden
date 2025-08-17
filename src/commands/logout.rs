use anyhow::{Context as _, Result, bail};
use colored::Colorize as _;
use tracing::instrument;

use crate::commands::common::{
    collect_all_pairs, filter_pairs, labels_credential_host, sort_pairs, styled_error_line,
};
use crate::config::Hosts;
use crate::utils::select_index;

#[instrument(skip(hosts_config))]
pub fn logout(
    hosts_config: &mut Hosts,
    hostname: Option<&String>,
    name: Option<&String>,
) -> Result<()> {
    let mut pairs = collect_all_pairs(hosts_config);
    if pairs.is_empty() {
        eprintln!("{}", styled_error_line("No credentials found to logout."));
        bail!("No credentials found to logout.");
    }
    sort_pairs(&mut pairs);

    let filtered = filter_pairs(
        &pairs,
        hostname.map(String::as_str),
        name.map(String::as_str),
    );
    if filtered.is_empty() {
        match (hostname, name) {
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
                let msg = "No credentials found to logout.".to_string();
                eprintln!("{}", styled_error_line(&msg));
                bail!(msg);
            },
        }
    }
    // decide which credential to operate on
    let target = if (hostname.is_some() && name.is_some()) || filtered.len() == 1 {
        filtered[0].clone()
    } else {
        let labels = labels_credential_host(&filtered);
        let prompt = match (hostname, name) {
            (Some(h), None) => format!("Select a credential to logout on {h}"),
            (None, Some(n)) => format!("Select a host to logout for '{n}'"),
            _ => "Select a credential to logout".to_string(),
        };
        let selection = select_index(&labels, prompt).context("Failed to select host")?;
        filtered[selection].clone()
    };
    if !hosts_config
        .remove_credential(&target.host, &target.credential)
        .context("Failed to remove credential from hosts configuration")?
    {
        let msg = format!(
            "Failed to remove credential {} for host {} from hosts configuration.",
            target.credential, target.host
        );
        eprintln!("{}", styled_error_line(&msg));
        bail!(msg);
    }
    eprintln!(
        "Successfully logged out {} {}",
        target.credential,
        format!("({})", target.host).dimmed()
    );
    Ok(())
}
