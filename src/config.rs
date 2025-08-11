use std::collections::HashMap;
use std::fs;

use anyhow::{Context as _, Result};
use config::{Config, File};
use serde::{Deserialize, Serialize};

use crate::keyring::erase_keyring_token;
use crate::profile::Profiles;
use crate::profile::rule::Rules;
use crate::profile::url::Patterns;
use crate::utils::config_dir;

/// Configuration for a single OAuth provider
#[derive(Clone, Debug, Deserialize)]
pub struct ProviderConfig {
    pub client_id: String,
    pub client_secret: String,
    pub auth_url: String,
    pub token_url: String,
    /// Device authorization endpoint (if provider supports device flow)
    pub device_auth_url: Option<String>,
    /// Requested OAuth scopes
    pub scopes: Vec<String>,
    /// Optional override: "auto", "device", or "authcode"
    pub preferred_flow: Option<String>,
}

/// OAuth configuration for the application
#[derive(Clone, Debug, Deserialize)]
pub struct OAuthConfig {
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    pub port: Option<u16>,
    pub oauth_only: Option<bool>,
}

impl OAuthConfig {
    /// Load configuration from standard config directory
    pub fn load() -> Result<Self> {
        let config_file = config_dir()?.join("oauth.toml");
        let builder = Config::builder().add_source(File::from(config_file).required(false));
        let settings = builder.build().context("Failed to build configuration")?;
        let cfg: Self = settings
            .try_deserialize()
            .context("Malformed configuration file")?;
        Ok(cfg)
    }
}

/// Configuration for profiles
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ProfileConfig {
    #[serde(default, alias = "patterns")]
    pub patterns: Patterns,
    #[serde(default)]
    pub profiles: Profiles,
    #[serde(default)]
    pub rules: Rules,
}

impl ProfileConfig {
    /// Load profile configuration from standard config directory
    pub fn load() -> Result<Self> {
        let config_file = config_dir()?.join("profiles.toml");
        let builder = Config::builder().add_source(File::from(config_file).required(false));
        let settings = builder
            .build()
            .context("Failed to build profile configuration")?;
        let cfg: Self = settings
            .try_deserialize()
            .context("Malformed profile configuration file")?;
        Ok(cfg)
    }
}

/// Configuration for hosts and their associated users/credentials
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HostConfig {
    pub active: String,
    pub users: Vec<String>,
}

/// Configuration for multiple hosts
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Hosts {
    inner: HashMap<String, HostConfig>,
}

impl Hosts {
    /// Load auth configuration from standard config directory
    pub fn load() -> Result<Self> {
        let config_file = config_dir()?.join(".hosts.toml");
        let builder = Config::builder().add_source(File::from(config_file).required(false));
        let settings = builder
            .build()
            .context("Failed to build auth configuration")?;

        if let Ok(flat) = settings
            .clone()
            .try_deserialize::<HashMap<String, HostConfig>>()
        {
            return Ok(Self::from_map(flat));
        }

        // nested like { "github": { "com": HostConfig } }
        let nested: HashMap<String, HashMap<String, HostConfig>> = settings
            .try_deserialize()
            .context("Malformed hosts configuration file")?;

        let flat = nested
            .into_iter()
            .flat_map(|(a, inner)| inner.into_iter().map(move |(b, v)| (format!("{a}.{b}"), v)))
            .collect();

        Ok(Self::from_map(flat))
    }

    /// Write the current configuration to the standard config directory.
    pub fn write(&self) -> Result<()> {
        let config_file = config_dir()?.join("hosts.toml");
        let toml = self.to_toml_string()?;
        fs::write(config_file, toml).context("Failed to write hosts configuration")?;
        Ok(())
    }

    /// Build from an existing map.
    pub const fn from_map(map: HashMap<String, HostConfig>) -> Self {
        Self { inner: map }
    }

    /// Serialize to pretty TOML.
    pub fn to_toml_string(&self) -> Result<String> {
        Ok(toml::to_string_pretty(&self.inner)?)
    }

    /// Returns true if there are no hosts configured.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn iter_sorted(&self) -> impl Iterator<Item = (&str, &HostConfig)> {
        let mut hosts: Vec<_> = self.inner.iter().collect();
        hosts.sort_by(|(a, _), (b, _)| a.cmp(b));
        hosts.into_iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn get_active_credential(&self, host: &str) -> Option<&str> {
        self.inner.get(host).map(|h| h.active.as_str())
    }

    pub fn get_users(&self, host: &str) -> Option<&[String]> {
        self.inner.get(host).map(|h| h.users.as_slice())
    }

    pub fn has_user(&self, host: &str, user: &str) -> bool {
        self.inner
            .get(host)
            .is_some_and(|h| h.users.iter().any(|u| u == user))
    }

    /// Get an iterator over all hosts and their configurations.
    pub fn hosts(&self) -> impl Iterator<Item = (&str, &HostConfig)> {
        self.inner.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Set the active user for a host and ensure it's present in `users`.
    pub fn set_active_credential(&mut self, host: &str, user: &str) -> Result<()> {
        let entry = self.inner.entry(host.to_string()).or_insert_with(|| {
            HostConfig {
                active: user.to_string(),
                users: vec![],
            }
        });
        entry.active = user.to_string();
        if !entry.users.iter().any(|u| u == user) {
            entry.users.push(user.to_string());
        }
        self.write()
    }

    /// Add a user to a host (no-op if already present). Returns true if added.
    pub fn add_user(&mut self, host: &str, user: &str) -> Result<bool> {
        let entry = self.inner.entry(host.to_string()).or_insert_with(|| {
            HostConfig {
                active: user.to_string(),
                users: vec![],
            }
        });
        if entry.users.iter().any(|u| u == user) {
            Ok(false)
        } else {
            entry.users.push(user.to_string());
            self.write()?;
            Ok(true)
        }
    }

    /// Remove a user. If it was active and others remain, switch to the first
    /// remaining.
    pub fn remove_user(&mut self, host: &str, user: &str) -> Result<bool> {
        let Some(entry) = self.inner.get_mut(host) else {
            return Ok(false);
        };

        let orig_len = entry.users.len();
        entry.users.retain(|u| u != user);
        let _ = erase_keyring_token(user, host);
        let removed = entry.users.len() != orig_len;

        // user was removed; if they were active, either promote someone else or drop
        // the host
        if removed {
            if entry.active == user {
                if let Some(new_active) = entry.users.first().cloned() {
                    entry.active = new_active;
                } else {
                    self.inner.remove(host);
                }
            }
            self.write()?;
        }

        Ok(removed)
    }

    /// Get a mutable reference to a host configuration.
    /// Note that changes made through this reference will not be persisted
    /// automatically!
    #[expect(dead_code, reason = "Keeping this for future use")]
    pub fn get_mut(&mut self, host: &str) -> Option<&mut HostConfig> {
        self.inner.get_mut(host)
    }

    /// Consume into the underlying map.
    /// Note that this will not persist any changes made to the map!
    #[expect(dead_code, reason = "Keeping this for future use")]
    pub fn into_inner(self) -> HashMap<String, HostConfig> {
        self.inner
    }
}

impl IntoIterator for Hosts {
    type Item = (String, HostConfig);
    type IntoIter = std::collections::hash_map::IntoIter<String, HostConfig>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}
