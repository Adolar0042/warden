use anyhow::{Context as _, Result};
use config::{Config, File};
use serde::Deserialize;

use crate::config::LoadableConfig;
use crate::profile::Profiles;
use crate::profile::rule::Rules;
use crate::profile::url::Patterns;
use crate::utils::config_dir;

/// Profiles / rules / patterns configuration.
///
/// Fields:
/// * `patterns` - Repository URL parsing patterns
/// * `profiles` - Named profile definitions (git config key to value maps)
/// * `rules` - Rules for matching repository URLs to profiles
///
/// Deserialization is intentionally lenient, unknown keys are ignored by
/// `config`/`serde`
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ProfileConfig {
    #[serde(default)]
    pub patterns: Patterns,
    #[serde(default)]
    pub profiles: Profiles,
    #[serde(default)]
    pub rules: Rules,
}

impl LoadableConfig for ProfileConfig {
    const KIND: &'static str = "profile";

    /// Load profile configuration from standard config directors. Missing file
    /// is an error.
    fn load_raw() -> Result<Self> {
        let path = config_dir()?.join("profiles.toml");
        let builder = Config::builder().add_source(File::from(path).required(true));
        let settings = builder
            .build()
            .context("Failed to build profile configuration")?;
        let cfg: Self = settings
            .try_deserialize()
            .context("Malformed profile configuration file")?;
        Ok(cfg)
    }
}
