// Portions of this file are derived from: https://github.com/siketyan/ghr
// Copyright (c) 2022 Naoki Ikeguchi
// Licensed under the MIT License. See LICENSES/MIT-ghr-UPSTREAM.md for details.
//
// Local modifications:
// Copyright (c) 2025 Adolar0042

use anyhow::Result;
use colored::Colorize as _;
use tracing::instrument;

use crate::config::ProfileConfig;

const INHERIT: &str = "(inherit)";

#[instrument(skip(profile_config))]
pub fn list(short: bool, profile_config: &ProfileConfig) -> Result<()> {
    profile_config.profiles.iter().for_each(|(name, profile)| {
        if short {
            println!("{name}");
        } else {
            println!(
                "  {}: {} {}",
                name.bold(),
                profile
                    .configs
                    .get("user.name")
                    .map_or(INHERIT, |name| name.as_str()),
                &format!(
                    "<{}>",
                    profile
                        .configs
                        .get("user.email")
                        .map_or(INHERIT, |email| email.as_str()),
                )
                .dimmed()
            );
        }
    });
    Ok(())
}
