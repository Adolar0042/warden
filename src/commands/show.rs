// Portions of this file are derived from: https://github.com/siketyan/ghr
// Copyright (c) 2022 Naoki Ikeguchi
// Licensed under the MIT License. See LICENSES/MIT-ghr-UPSTREAM.md for details.
//
// Local modifications:
// Copyright (c) 2025 Adolar0042

use anyhow::{Result, bail};
use tracing::instrument;

use crate::commands::common::styled_error;
use crate::config::ProfileConfig;
use crate::load_cfg;
use crate::profile::rule::ProfileRef;

#[instrument]
pub fn show(profile_ref: &ProfileRef) -> Result<()> {
    let profile_config = load_cfg!(ProfileConfig)?;
    let Some(profile) = profile_config.profiles.get(&profile_ref.name) else {
        styled_error(format!("Unknown profile: {}", &profile_ref.name));
        bail!("Unknown profile: {}", &profile_ref.name);
    };

    for (k, v) in &profile.configs {
        println!("{k} = \"{v}\"");
    }

    Ok(())
}
