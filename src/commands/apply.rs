// Portions of this file are derived from: https://github.com/siketyan/ghr
// Copyright (c) 2022 Naoki Ikeguchi
// Licensed under the MIT License. See LICENSES/MIT-ghr-UPSTREAM.md for details.
//
// Local modifications:
// Copyright (c) 2025 Adolar0042

use anyhow::{Result, anyhow, bail};
use colored::Colorize as _;
use git2::Repository;
use tracing::instrument;

use crate::commands::common::styled_error;
use crate::config::ProfileConfig;
use crate::load_cfg;
use crate::profile::rule::ProfileRef;
use crate::profile::url::{Patterns, Url as RepoUrl};

const INHERIT: &str = "(inherit)";

#[instrument]
pub fn apply(profile_name: Option<String>) -> Result<()> {
    let profile_config = load_cfg!(ProfileConfig)?;
    if let Some(name) = profile_name {
        let profile_ref = ProfileRef { name };
        let profile = profile_config
            .profiles
            .get(&profile_ref.name)
            .ok_or_else(|| anyhow!("Unknown profile: {}", &profile_ref.name))?;

        profile.apply()?;

        eprintln!("Attached profile {} successfully.", profile_ref.name.bold());
    } else {
        let repo = Repository::open_from_env();
        let Ok(repo) = repo else {
            styled_error("Not a git repository!");
            bail!("Not a git repository!");
        };

        let remote = repo.find_remote("origin");
        let Ok(remote) = remote else {
            styled_error("No remote named 'origin' found");
            bail!("No remote named 'origin' found");
        };
        let remote_url = remote.url().expect("No remote url");
        let url: RepoUrl = match RepoUrl::from_str(remote_url, &profile_config.patterns, None) {
            Ok(u) => u,
            Err(_) => RepoUrl::from_str(remote_url, &Patterns::default(), None)?,
        };

        let rule = profile_config.rules.resolve(&url);
        match rule {
            None => {
                styled_error(format!(
                    "No profile found for [{}].",
                    &url.to_string().bold()
                ));
                bail!("No rule matched for remote {}", &url.to_string());
            },
            Some(rule) => {
                let profile = profile_config
                    .profiles
                    .resolve(&rule.profile)
                    .expect("No profile found");
                profile.1.apply()?;
                eprintln!("Attached profile {} successfully.", profile.0.bold());
                println!(
                    "  {}: {} {}",
                    profile.0.bold(),
                    profile
                        .1
                        .configs
                        .get("user.name")
                        .map_or(INHERIT, |name| name.as_str()),
                    &format!(
                        "<{}>",
                        profile
                            .1
                            .configs
                            .get("user.email")
                            .map_or(INHERIT, |email| email.as_str()),
                    )
                    .dimmed(),
                );
            },
        }
    }

    Ok(())
}
