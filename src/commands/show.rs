// Portions of this file are derived from: https://github.com/siketyan/ghr
// Copyright (c) 2022 Naoki Ikeguchi
// Licensed under the MIT License. See LICENSES/MIT-ghr-UPSTREAM.md for details.
//
// Local modifications:
// Copyright (c) 2025 Adolar0042

use anyhow::{Result, anyhow};
use tracing::instrument;

use crate::config::ProfileConfig;
use crate::profile::rule::ProfileRef;

#[instrument]
pub fn show(profile_ref: &ProfileRef, profile_config: &ProfileConfig) -> Result<()> {
    let profile = profile_config
        .profiles
        .get(&profile_ref.name)
        .ok_or_else(|| anyhow!("Unknown profile: {}", &profile_ref.name))?;

    for (k, v) in &profile.configs {
        println!("{k} = \"{v}\"");
    }

    Ok(())
}
