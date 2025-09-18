use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use config::{Config, File};
use serde::Deserialize;
use tracing::warn;
use url::Url;

use crate::config::LoadableConfig;
use crate::config::git_source::GitConfigSource;
use crate::utils::config_dir;

struct ProviderDefaults {
    auth_path: &'static str,
    token_path: &'static str,
    device_auth_path: Option<&'static str>,
    scopes: &'static [&'static str],
    preferred_flow: &'static str,
}

const GITHUB: ProviderDefaults = ProviderDefaults {
    auth_path: "/login/oauth/authorize",
    token_path: "/login/oauth/access_token",
    device_auth_path: Some("/login/device/code"),
    scopes: &["repo", "read:org", "write:org", "workflow"],
    preferred_flow: "authcode",
};
const GITLAB: ProviderDefaults = ProviderDefaults {
    auth_path: "/oauth/authorize",
    token_path: "/oauth/token",
    device_auth_path: Some("/oauth/authorize_device"),
    scopes: &["read_repository", "write_repository"],
    preferred_flow: "authcode",
};
const FORGEJO: ProviderDefaults = ProviderDefaults {
    auth_path: "/login/oauth/authorize",
    token_path: "/login/oauth/access_token",
    device_auth_path: None,
    scopes: &["read:repository", "write:repository"],
    preferred_flow: "authcode",
};

/// Configuration for a single OAuth provider.
///
/// Fields:
/// - `type`: Optional, gives defaults for URLs and scopes. Known values:
///   "github", "gitlab", "forgejo", "gitea". If omitted, `auth_url` and
///   `token_url` must be provided.
/// - `client_id`: Required, empty strings are treated as invalid
/// - `client_secret`: Optional (PKCE auth-code flow often does not need it)
/// - `auth_url`, `token_url`: Optional; filled from provider type when omitted.
///   If provided, must be absolute URLs or start with "/" (validated)
/// - `device_auth_url`: Optional device authorization endpoint (validated if
///   present)
/// - `scopes`: Optional list of scopes. `None` => do not send a `scope`
///   parameter. `Some(empty)` => explicitly send an empty scope set (depends on
///   OAuth server behavior)
/// - `preferred_flow`: Optional override ("auto" | "device" | "authcode")
#[derive(Clone, Debug, Deserialize)]
pub struct ProviderConfig {
    #[serde(alias = "type")]
    pub provider_type: Option<String>,
    pub client_id: String,
    pub client_secret: Option<String>,
    #[serde(default)]
    pub auth_url: String,
    #[serde(default)]
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
/// 1. oauth.toml
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

impl LoadableConfig for OAuthConfig {
    const KIND: &'static str = "OAuth";

    /// Load and merge configuration sources
    fn load_raw() -> Result<Self> {
        let config_file = config_dir()?.join("oauth.toml");

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

fn provider_endpoint_base(name: &str) -> String {
    if name.starts_with("http://") || name.starts_with("https://") {
        name.to_string()
    } else {
        format!("https://{name}")
    }
}

fn resolve_endpoint(base: &str, v: &str) -> String {
    if v.starts_with('/') {
        format!("{base}{v}")
    } else {
        v.to_string()
    }
}

fn apply_type_defaults(provider: &mut ProviderConfig, ptype: &str, errs: &mut Vec<String>) {
    let defaults = match ptype.to_lowercase().as_str() {
        "github" => Some(&GITHUB),
        "gitlab" => Some(&GITLAB),
        "forgejo" | "gitea" => Some(&FORGEJO),
        _ => None,
    };

    if let Some(defaults) = defaults {
        if provider.auth_url.trim().is_empty() {
            provider.auth_url = defaults.auth_path.to_string();
        }
        if provider.token_url.trim().is_empty() {
            provider.token_url = defaults.token_path.to_string();
        }
        match (&mut provider.device_auth_url, defaults.device_auth_path) {
            (url @ None, Some(path)) => {
                *url = Some(path.to_string());
            },
            (Some(url), Some(path)) if url.trim().is_empty() => {
                *url = path.to_string();
            },
            _ => {},
        }
        if provider.scopes.is_none() || provider.scopes.as_ref().unwrap().is_empty() {
            provider.scopes = Some(
                defaults
                    .scopes
                    .iter()
                    .map(|scope| (*scope).to_string())
                    .collect(),
            );
        }
        if provider.preferred_flow.is_none() {
            provider.preferred_flow = Some(defaults.preferred_flow.to_string());
        }
    } else {
        errs.push("unknown provider type".to_string());
    }
}

fn validate_and_normalize_provider(name: &str, provider: &mut ProviderConfig) -> Vec<String> {
    let mut errs = Vec::new();
    let endpoint_base = provider_endpoint_base(name);

    if let Some(ptype) = provider.provider_type.clone() {
        if ptype.trim().is_empty()
            && (provider.auth_url.trim().is_empty() || provider.token_url.trim().is_empty())
        {
            errs.push("missing provider_type or auth_url/token_url".to_string());
        }
        apply_type_defaults(provider, &ptype, &mut errs);
    }

    if provider.client_id.trim().is_empty() {
        errs.push("missing client_id".into());
    }

    if provider.auth_url.trim().is_empty() {
        errs.push("missing auth_url".into());
    } else {
        provider.auth_url = resolve_endpoint(&endpoint_base, &provider.auth_url);
        if Url::parse(&provider.auth_url).is_err() {
            errs.push("invalid auth_url".into());
        }
    }

    if provider.token_url.trim().is_empty() {
        errs.push("missing token_url".into());
    } else {
        provider.token_url = resolve_endpoint(&endpoint_base, &provider.token_url);
        if Url::parse(&provider.token_url).is_err() {
            errs.push("invalid token_url".into());
        }
    }

    if let Some(url) = provider.device_auth_url.as_mut() {
        let resolved = resolve_endpoint(&endpoint_base, url);
        *url = resolved;
        if Url::parse(url.as_str()).is_err() {
            errs.push("invalid device_auth_url".into());
        }
    }

    errs
}

/// Validate provider entries and discard invalid ones, logging warnings
fn validate_providers(mut cfg: OAuthConfig) -> Result<OAuthConfig> {
    let mut invalid: Vec<(String, Vec<String>)> = Vec::new();

    for (name, provider) in &mut cfg.providers {
        let errs = validate_and_normalize_provider(name, provider);
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
                        provider_type: None,
                        client_id: "some-id".into(),
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
                        provider_type: None,
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

    #[test]
    fn provider_type_gitlab_fills_defaults() {
        let cfg = OAuthConfig {
            providers: HashMap::from_iter([(
                "example.com".into(),
                ProviderConfig {
                    provider_type: Some("gitlab".into()),
                    client_id: "some-id".into(),
                    client_secret: None,
                    auth_url: String::new(),
                    token_url: String::new(),
                    device_auth_url: None,
                    scopes: None,
                    preferred_flow: None,
                },
            )]),
            port: None,
            oauth_only: None,
        };

        let cfg = validate_providers(cfg).unwrap();

        let p = &cfg.providers["example.com"];
        assert_eq!(p.auth_url, "https://example.com/oauth/authorize");
        assert_eq!(p.token_url, "https://example.com/oauth/token");
        assert_eq!(
            p.device_auth_url.as_deref(),
            Some("https://example.com/oauth/authorize_device")
        );
        assert_eq!(p.preferred_flow.as_deref(), Some("authcode"));
        assert_eq!(
            p.scopes.as_ref().unwrap(),
            &vec![
                "read_repository".to_string(),
                "write_repository".to_string(),
            ]
        );
    }

    #[test]
    fn provider_type_respects_overrides() {
        let cfg = OAuthConfig {
            providers: HashMap::from_iter([(
                // this somehow is a valid domain name
                "example".into(),
                ProviderConfig {
                    provider_type: Some("forgejo".into()),
                    client_id: "some-id".into(),
                    client_secret: None,
                    auth_url: "https://override.example/custom_auth".into(),
                    token_url: String::new(),
                    device_auth_url: Some("/custom/device".into()),
                    scopes: None,
                    preferred_flow: None,
                },
            )]),
            port: None,
            oauth_only: None,
        };

        let cfg = validate_providers(cfg).unwrap();

        let p = &cfg.providers["example"];
        assert_eq!(p.auth_url, "https://override.example/custom_auth");
        assert_eq!(p.token_url, "https://example/login/oauth/access_token");
        assert_eq!(
            p.device_auth_url.as_deref(),
            Some("https://example/custom/device")
        );
        assert_eq!(p.preferred_flow.as_deref(), Some("authcode"));
        assert_eq!(
            p.scopes.as_ref().unwrap(),
            &vec![
                "read:repository".to_string(),
                "write:repository".to_string(),
            ]
        );
    }

    #[test]
    fn scheme_in_key_resolved() {
        let cfg = OAuthConfig {
            providers: HashMap::from_iter([(
                "https://gitlab.example.com".into(),
                ProviderConfig {
                    provider_type: Some("gitlab".into()),
                    client_id: "some-id".into(),
                    client_secret: None,
                    auth_url: String::new(),
                    token_url: String::new(),
                    device_auth_url: None,
                    scopes: None,
                    preferred_flow: None,
                },
            )]),
            port: None,
            oauth_only: None,
        };

        let cfg = validate_providers(cfg).unwrap();

        let p = &cfg.providers["https://gitlab.example.com"];
        assert_eq!(p.auth_url, "https://gitlab.example.com/oauth/authorize");
        assert_eq!(p.token_url, "https://gitlab.example.com/oauth/token");
        assert_eq!(
            p.device_auth_url.as_deref(),
            Some("https://gitlab.example.com/oauth/authorize_device")
        );
        assert_eq!(p.preferred_flow.as_deref(), Some("authcode"));
        assert_eq!(
            p.scopes.as_ref().unwrap(),
            &vec![
                "read_repository".to_string(),
                "write_repository".to_string(),
            ]
        );
    }

    #[test]
    fn empty_providers_error() {
        let cfg = OAuthConfig {
            providers: HashMap::new(),
            port: None,
            oauth_only: None,
        };
        validate_providers(cfg).unwrap_err();
    }
}
