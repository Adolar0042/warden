// Portions of this file are derived from: https://github.com/siketyan/ghr
// Copyright (c) 2022 Naoki Ikeguchi
// Licensed under the MIT License. See LICENSES/MIT-ghr-UPSTREAM.md for details.
//
// Local modifications:
// Copyright (c) 2025 Adolar0042

use anyhow::{Result, anyhow};
use colored::Colorize as _;
use git2::Repository;
use tracing::instrument;

use crate::commands::common::styled_error_line;
use crate::config::ProfileConfig;
use crate::profile::rule::ProfileRef;
use crate::profile::url::{Patterns, Url as RepoUrl};

const INHERIT: &str = "(inherit)";

#[instrument(skip(profile_config))]
pub fn apply(profile_name: Option<String>, profile_config: &ProfileConfig) -> Result<()> {
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
            eprintln!("{}", styled_error_line("Not a git repository!"));
            return Ok(());
        };

        let remote = repo.find_remote("origin");
        let Ok(remote) = remote else {
            eprintln!("{}", styled_error_line("No remote named 'origin' found"));
            return Ok(());
        };
        let remote_url = remote.url().expect("No remote url");
        let url: RepoUrl = match RepoUrl::from_str(remote_url, &profile_config.patterns, None) {
            Ok(u) => u,
            Err(_) => RepoUrl::from_str(remote_url, &Patterns::default(), None)?,
        };

        let rule = profile_config.rules.resolve(&url);
        match rule {
            None => {
                eprintln!(
                    "{}",
                    styled_error_line(format!(
                        "No profile found for [{}].",
                        &url.to_string().bold()
                    ))
                );
                return Ok(());
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
