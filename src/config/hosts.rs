use std::collections::HashMap;
use std::fs;

use anyhow::{Context as _, Result};
use config::{Config, File};
use serde::{Deserialize, Serialize};

use crate::keyring::erase_keyring_token;
use crate::utils::config_dir;

/// Represents the stored state for a single host and its credentials
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HostConfig {
    /// Currently active credential for this host
    pub active: String,
    /// All known credentials for this host
    #[serde(alias = "users")]
    pub credentials: Vec<String>,
}

/// Collection of hosts keyed by their fully-qualified hostname
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Hosts {
    inner: HashMap<String, HostConfig>,
}

impl Hosts {
    /// Load host states from the standard config directory
    ///
    /// The on-disk format is an (optionally nested) TOML map stored in
    /// `.hosts.toml`:
    ///
    /// ```toml
    /// [github.com]
    /// active = "alice"
    /// users = ["alice", "bob"]
    ///
    /// [gitlab.example.com]
    /// active = "carol"
    /// users = ["carol"]
    /// ```
    pub fn load() -> Result<Self> {
        let path = config_dir()?.join(".hosts.toml");
        let builder = Config::builder().add_source(File::from(path).required(false));
        let settings = builder
            .build()
            .context("Failed to build hosts configuration")?;

        // first try the straightforward flat map form
        // (with lots of hopium)
        if let Ok(flat) = settings
            .clone()
            .try_deserialize::<HashMap<String, HostConfig>>()
        {
            return Ok(Self { inner: flat });
        }

        // Fallback: recursively flatten arbitrary nesting
        let value: serde_json::Value = settings
            .try_deserialize()
            .context("Malformed hosts configuration file")?;

        let mut flat: HashMap<String, HostConfig> = HashMap::new();
        Self::flatten_hosts("", &value, &mut flat)
            .context("Failed to flatten nested hosts configuration")?;
        Ok(Self::from_map(flat))
    }

    fn flatten_hosts(
        prefix: &str,
        v: &serde_json::Value,
        out: &mut HashMap<String, HostConfig>,
    ) -> Result<()> {
        if v.is_object() {
            // attempt direct HostConfig deserialization first
            if let Ok(host_cfg) = serde_json::from_value::<HostConfig>(v.clone()) {
                if !prefix.is_empty() {
                    out.insert(prefix.to_string(), host_cfg);
                }
                return Ok(());
            }
            for (k, child) in v.as_object().unwrap() {
                let next = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                Self::flatten_hosts(&next, child, out)?;
            }
        }
        Ok(())
    }

    /// Write the current state to the standard config directory
    pub fn write(&self) -> Result<()> {
        let path = config_dir()?.join(".hosts.toml");
        let toml = self.to_toml_string()?;
        fs::write(&path, toml).context("Failed to write hosts state")?;
        Ok(())
    }

    /// Serialize to pretty TOML
    pub fn to_toml_string(&self) -> Result<String> {
        Ok(toml::to_string_pretty(&self.inner)?)
    }

    /// Construct from an existing map (does not write to disk)
    pub const fn from_map(map: HashMap<String, HostConfig>) -> Self {
        Self { inner: map }
    }

    /// Returns true if no hosts are recorded
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Iterate over hosts sorted by hostname
    pub fn iter_sorted(&self) -> impl Iterator<Item = (&str, &HostConfig)> {
        let mut items: Vec<_> = self.inner.iter().collect();
        items.sort_by(|(a, _), (b, _)| a.cmp(b));
        items.into_iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Get the active credential for a host if it exists
    pub fn get_active_credential(&self, host: &str) -> Option<&str> {
        self.inner.get(host).map(|h| h.active.as_str())
    }

    /// Get list of all credentials for a host
    pub fn get_credentials(&self, host: &str) -> Option<&[String]> {
        self.inner.get(host).map(|h| h.credentials.as_slice())
    }

    /// True if `credential` is present for `host`
    pub fn has_credential(&self, host: &str, credential: &str) -> bool {
        self.inner
            .get(host)
            .is_some_and(|h| h.credentials.iter().any(|u| u == credential))
    }

    /// Iterate over (host, state) pairs in arbitrary order
    pub fn hosts(&self) -> impl Iterator<Item = (&str, &HostConfig)> {
        self.inner.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Set the active credential for a host, inserting the host if missing
    /// Ensures the credential is present in the `crednetials` list
    pub fn set_active_credential(&mut self, host: &str, credential: &str) -> Result<()> {
        let entry = self.inner.entry(host.to_string()).or_insert_with(|| {
            HostConfig {
                active: credential.to_string(),
                credentials: vec![],
            }
        });
        entry.active = credential.to_string();
        if !entry.credentials.iter().any(|u| u == credential) {
            entry.credentials.push(credential.to_string());
        }
        self.write()
    }

    /// Add a credential to a host. Returns `true` if it was newly inserted
    pub fn add_credential(&mut self, host: &str, credential: &str) -> Result<bool> {
        let entry = self.inner.entry(host.to_string()).or_insert_with(|| {
            HostConfig {
                active: credential.to_string(),
                credentials: vec![],
            }
        });
        if entry.credentials.iter().any(|u| u == credential) {
            Ok(false)
        } else {
            entry.credentials.push(credential.to_string());
            self.write()?;
            Ok(true)
        }
    }

    /// Remove a credential; if it was the active credential and others remain,
    /// the first remaining credential becomes active. If no credentials
    /// remain the host entry is removed. Returns whether removal occurred.
    pub fn remove_credential(&mut self, host: &str, credential: &str) -> Result<bool> {
        let Some(entry) = self.inner.get_mut(host) else {
            return Ok(false);
        };
        let original_len = entry.credentials.len();
        entry.credentials.retain(|u| u != credential);
        let _ = erase_keyring_token(credential, host);
        let removed = entry.credentials.len() != original_len;

        if removed {
            if entry.active == credential {
                if let Some(first) = entry.credentials.first().cloned() {
                    entry.active = first;
                } else {
                    // No credentialss left: drop the host entry entirely.
                    self.inner.remove(host);
                }
            }
            self.write()?;
        }
        Ok(removed)
    }

    /// Get mutable access to a host's state (non-persisted).
    #[expect(dead_code, reason = "Keeping for future use")]
    pub fn get_mut(&mut self, host: &str) -> Option<&mut HostConfig> {
        self.inner.get_mut(host)
    }

    /// Consume and return the underlying map.
    #[expect(dead_code, reason = "Keeping for future use")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_nested_hosts() {
        let json = serde_json::json!({
            "gitlab": {
                "example": {
                    "com": {
                        "active": "carol",
                        "users": ["carol"]
                    }
                }
            }
        });
        let mut out = HashMap::new();
        Hosts::flatten_hosts("", &json, &mut out).unwrap();
        assert!(out.contains_key("gitlab.example.com"));
        assert_eq!(out["gitlab.example.com"].active, "carol");
    }
}
