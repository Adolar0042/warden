use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use config::{Config, File};
use serde::Deserialize;
use tracing::warn;
use url::Url;

use crate::config::git_source::GitConfigSource;

/// Configuration for a single OAuth provider.
///
/// Fields:
/// - `client_id`: Required; empty strings are treated as invalid.
/// - `client_secret`: Optional (PKCE auth-code flow often does not need it).
/// - `auth_url`, `token_url`: Required absolute URLs (validated).
/// - `device_auth_url`: Optional device authorization endpoint (validated if
///   present).
/// - `scopes`: Optional list of scopes. `None` => do not send a `scope`
///   parameter. `Some(empty)` => explicitly send an empty scope set (depends on
///   OAuth server behavior).
/// - `preferred_flow`: Optional override ("auto" | "device" | "authcode").
#[derive(Clone, Debug, Deserialize)]
pub struct ProviderConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub auth_url: String,
    pub token_url: String,
    /// Device authorization endpoint, if supported by the provider
    pub device_auth_url: Option<String>,
    /// Optional scopes to request during authorization
    pub scopes: Option<Vec<String>>,
    // Optional override: "auto", "device" or "authcode"
    pub preferred_flow: Option<String>,
}

/// OAuth configurations for various providers.
///
/// Loaded from (in precedence order where later overrides earlier):
/// 1. oauth.toml (optional)
/// 2. Global/system/user Git configuration
/// 3. Repository-local Git configuration
///
/// After merging, providers are validated and invalid ones are discarded,
/// emitting a warning of what is wrong.
#[derive(Clone, Debug, Deserialize)]
pub struct OAuthConfig {
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    pub port: Option<u16>,
    pub oauth_only: Option<bool>,
}

impl OAuthConfig {
    /// Load and merge configuration sources
    pub fn load() -> Result<Self> {
        let config_file = crate::utils::config_dir()?.join("oauth.toml");

        let builder = Config::builder()
            .add_source(File::from(config_file).required(false))
            .add_source(GitConfigSource::global())
            .add_source(GitConfigSource::repo());

        let settings = builder
            .build()
            .context("Failed to build configuration for OAuth providers")?;

        let cfg: Self = settings
            .try_deserialize()
            .context("Malformed OAuth provider configuration")?;

        let cfg = validate_providers(cfg).context("Invalid OAuth provider configuration")?;
        Ok(cfg)
    }
}

/// Validate provider entries and discard invalid ones, logging warnings instead
/// of panicking later during OAuth flows.
fn validate_providers(mut cfg: OAuthConfig) -> Result<OAuthConfig> {
    let mut invalid: Vec<(String, Vec<String>)> = Vec::new();

    for (name, provider) in &cfg.providers {
        let mut errs = Vec::new();

        if provider.client_id.trim().is_empty() {
            errs.push("missing client_id".into());
        }
        if Url::parse(&provider.auth_url).is_err() {
            errs.push("invalid auth_url".into());
        }
        if Url::parse(&provider.token_url).is_err() {
            errs.push("invalid token_url".into());
        }
        if let Some(url) = &provider.device_auth_url
            && Url::parse(url).is_err()
        {
            errs.push("invalid device_auth_url".into());
        }

        if !errs.is_empty() {
            invalid.push((name.clone(), errs));
        }
    }

    if !invalid.is_empty() {
        for (name, errs) in &invalid {
            warn!(
                "Discarding invalid OAuth provider '{name}': {}",
                errs.join(", ")
            );
        }
        for (name, _) in invalid {
            cfg.providers.remove(&name);
        }
    }
    if cfg.providers.is_empty() {
        bail!("No valid OAuth providers configured");
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_discards_invalid() {
        let cfg = OAuthConfig {
            providers: HashMap::from_iter([
                (
                    "good.example".into(),
                    ProviderConfig {
                        client_id: "abc".into(),
                        client_secret: None,
                        auth_url: "https://good.example/auth".into(),
                        token_url: "https://good.example/token".into(),
                        device_auth_url: None,
                        scopes: None,
                        preferred_flow: None,
                    },
                ),
                (
                    "bad.example".into(),
                    ProviderConfig {
                        client_id: String::new(),
                        client_secret: None,
                        auth_url: "notaurl".into(),
                        token_url: "https://still.ok/token".into(),
                        device_auth_url: Some("also_bad".into()),
                        scopes: Some(vec![]),
                        preferred_flow: None,
                    },
                ),
            ]),
            port: None,
            oauth_only: None,
        };

        let cfg = validate_providers(cfg).unwrap();
        assert!(cfg.providers.contains_key("good.example"));
        assert!(!cfg.providers.contains_key("bad.example"));
    }
}
