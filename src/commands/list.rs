// Portions of this file are derived from: https://github.com/siketyan/ghr
// Copyright (c) 2022 Naoki Ikeguchi
// Licensed under the MIT License. See LICENSES/MIT-ghr-UPSTREAM.md for details.
//
// Local modifications:
// Copyright (c) 2025 Adolar0042

use anyhow::{Result, bail};
use colored::Colorize as _;
use tracing::instrument;

use crate::commands::common::styled_error;
use crate::config::ProfileConfig;
use crate::load_cfg;

const INHERIT: &str = "(inherit)";

#[instrument]
pub fn list(short: bool) -> Result<()> {
    let profile_config = load_cfg!(ProfileConfig)?;
    if profile_config.profiles.is_empty() {
        styled_error("No profiles found");
        bail!("No profiles found");
    }
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
