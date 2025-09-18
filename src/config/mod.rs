//! Central configuration module.
//!
//! Splits up the individual configuration files into submodules for clarity.
//!
//! Layout:
//! - `provider`: OAuth provider configuration loading and validation
//! - `git_source`: `config::Source` implementation for Git-based provider
//!   overrides
//! - `hosts`: host/credential state
//! - `profiles`: profile, rule and pattern configuration

pub mod git_source;
pub mod hosts;
pub mod profiles;
pub mod provider;

use anyhow::{Context as _, Result};
pub use hosts::Hosts;
pub use profiles::ProfileConfig;
pub use provider::{OAuthConfig, ProviderConfig};

pub trait LoadableConfig: Sized {
    const KIND: &'static str;

    /// Load configuration from the standard config directory
    fn load() -> Result<Self> {
        Self::load_raw().context(format!("Failed to load {} configuration", Self::KIND))
    }

    fn load_raw() -> Result<Self>;
}

#[macro_export]
macro_rules! load_cfg {
    ($t:ty) => {
        <$t as $crate::config::LoadableConfig>::load()
    };
}
