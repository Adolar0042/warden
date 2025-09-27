// Portions of this file are derived from: https://github.com/siketyan/ghr
// Copyright (c) 2022 Naoki Ikeguchi
// Licensed under the MIT License. See LICENSES/MIT-ghr-UPSTREAM.md for details.
//
// Local modifications:
// Copyright (c) 2025 Adolar0042

use std::collections::HashMap;
use std::collections::hash_map::Iter;
use std::fmt::Formatter;
use std::ops::Deref;

use anyhow::{Context as _, Result, bail};
use git2::Repository;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use toml::{Table, Value};

use crate::profile::rule::ProfileRef;

pub mod rule;
pub mod url;

#[derive(Clone, Debug, Default)]
pub struct Configs {
    map: HashMap<String, String>,
}

impl Configs {
    /// Convert this flattened map to a nested TOML table structure.
    /// Returns an error if conflicting keys are encountered, e.g. when
    /// both "user" (as a value) and "user.name" (as a nested key) exist.
    fn to_toml(&self) -> Result<Table> {
        let mut root = Table::new();

        for (full_key, value) in &self.map {
            let segments: Vec<&str> = full_key.split('.').collect();
            if segments.is_empty() {
                continue;
            }

            // Walk or create nested tables for all but the last segment
            let mut current: &mut Table = &mut root;
            let mut path = String::new();

            for seg in &segments[..segments.len() - 1] {
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(seg);

                match current.get_mut(*seg) {
                    None => {
                        current.insert((*seg).to_string(), Value::Table(Table::new()));
                    },
                    Some(Value::Table(_)) => {}, // ok, descend
                    Some(_) => {
                        bail!(
                            "Conflicting key '{full_key}' at '{path}': expected a table but found \
                             a value",
                        );
                    },
                }

                // Safe to unwrap as we just inserted or confirmed it's a table
                current = match current.get_mut(*seg).unwrap() {
                    Value::Table(t) => t,
                    Value::String(_)
                    | Value::Integer(_)
                    | Value::Float(_)
                    | Value::Boolean(_)
                    | Value::Datetime(_)
                    | Value::Array(_) => unreachable!("Branch above guarantees a table"),
                };
            }

            // Insert the final value
            let last = segments[segments.len() - 1];
            match current.get(last) {
                Some(Value::Table(_)) => {
                    bail!("Conflicting key '{full_key}': cannot overwrite a table with a value",);
                },
                _ => {
                    current.insert(last.to_string(), Value::String(value.clone()));
                },
            }
        }

        Ok(root)
    }

    /// Extend the flattened map by reading the provided TOML value recursively.
    /// - Tables are traversed and keys are joined with '.'
    /// - Scalar values are stringified and inserted
    /// - Arrays are rejected (git config expects scalar values)
    fn extend_from_toml(&mut self, input: &Value, current_key: &str) -> Result<()> {
        match input {
            Value::Table(table) => {
                let mut keys: Vec<_> = table.keys().cloned().collect();
                keys.sort_unstable();
                for key in keys {
                    let value = table.get(&key).expect("key taken from same table");
                    let next_key = if current_key.is_empty() {
                        key
                    } else if key.is_empty() {
                        current_key.to_string()
                    } else {
                        format!("{current_key}.{key}")
                    };
                    self.extend_from_toml(value, &next_key)?;
                }
                Ok(())
            },
            Value::Array(_) => {
                bail!("Arrays are not supported in profile configs at key '{current_key}'",)
            },
            // All scalars: coerce to string (git config values are strings)
            other @ (Value::String(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::Boolean(_)
            | Value::Datetime(_)) => {
                let coerced = if let Value::String(s) = other {
                    s.clone()
                } else {
                    other.to_string()
                };
                self.map.insert(current_key.to_string(), coerced);
                Ok(())
            },
        }
    }
}

impl Deref for Configs {
    type Target = HashMap<String, String>;
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<'de> Deserialize<'de> for Configs {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ConfigsVisitor)
    }
}

impl Serialize for Configs {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let table = self
            .to_toml()
            .map_err(<S::Error as serde::ser::Error>::custom)?;
        table.serialize(serializer)
    }
}

struct ConfigsVisitor;

impl<'de> Visitor<'de> for ConfigsVisitor {
    type Value = Configs;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("git configs of a profile")
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut configs = Configs::default();

        while let Some((key, value)) = map
            .next_entry::<String, Value>()
            .map_err(<A::Error as serde::de::Error>::custom)?
        {
            configs
                .extend_from_toml(&value, &key)
                .map_err(<A::Error as serde::de::Error>::custom)?;
        }

        Ok(configs)
    }
}

/// A profile wraps a set of configuration entries.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Profile {
    #[serde(default, flatten)]
    pub configs: Configs,
}

impl Profile {
    /// Apply this profile's configurations to the current git repository
    /// config.
    pub fn apply(&self) -> Result<()> {
        let repo = Repository::open_from_env().context("Failed to open git repository")?;
        let mut cfg = repo.config().context("Failed to open git config")?;

        for (key, value) in &self.configs.map {
            cfg.set_str(key, value)
                .with_context(|| format!("Failed to set git config '{key}'"))?;
        }

        Ok(())
    }
}

/// A collection of named profiles.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Profiles {
    #[serde(default, flatten)]
    map: HashMap<String, Profile>,
}

impl Profiles {
    /// Resolve a profile reference to its name and associated `Profile`.
    pub fn resolve(&self, r: &ProfileRef) -> Option<(&str, &Profile)> {
        self.map
            .get_key_value(&r.name)
            .map(|(k, v)| (k.as_str(), v))
    }
}

impl Deref for Profiles {
    type Target = HashMap<String, Profile>;
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<'a> IntoIterator for &'a Configs {
    type Item = (&'a String, &'a String);
    type IntoIter = Iter<'a, String, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_git_configs_string_scalars() {
        let toml = r#"
        user.name = "User"
        user.email = "user@example.com"
        user.signingkey = "ABCDEFGHIJKLMNOP"
        "#;

        let profile = toml::from_str::<Profile>(toml).unwrap();
        let configs = &profile.configs;

        assert_eq!("User", configs.get("user.name").unwrap().as_str());
        assert_eq!(
            "user@example.com",
            configs.get("user.email").unwrap().as_str(),
        );
        assert_eq!(
            "ABCDEFGHIJKLMNOP",
            configs.get("user.signingkey").unwrap().as_str(),
        );
    }

    #[test]
    fn load_git_configs_non_string_scalars_are_coerced() {
        let toml = "
        core.filemode = false
        core.timeout = 30
        ";

        let profile = toml::from_str::<Profile>(toml).unwrap();
        let configs = &profile.configs;

        assert_eq!("false", configs.get("core.filemode").unwrap());
        assert_eq!("30", configs.get("core.timeout").unwrap());
    }

    #[test]
    fn reject_arrays_in_configs() {
        let toml = r#"
        core.excludesfile = ["a", "b"]
        "#;

        let res = toml::from_str::<Profile>(toml);
        assert!(res.is_err(), "arrays must be rejected");
    }

    #[test]
    fn deterministic_serialization_and_conflict_detection() {
        // Nested tables
        let mut cfgs = Configs::default();
        cfgs.map.insert("user.name".into(), "User".into());
        cfgs.map
            .insert("user.email".into(), "user@example.com".into());
        cfgs.map.insert("core.filemode".into(), "false".into());

        // Should serialize to nested TOML table
        let table = cfgs.to_toml().expect("no conflicts");
        let Value::Table(user) = table.get("user").unwrap() else {
            panic!("expected user to be a table")
        };
        assert_eq!(
            user.get("name").unwrap(),
            &Value::String("User".to_string())
        );
        assert_eq!(
            user.get("email").unwrap(),
            &Value::String("user@example.com".to_string())
        );

        // Introduce a conflict: a value at 'user' while we already need a table at
        // 'user'
        let mut bad = cfgs.clone();
        bad.map.insert("user".into(), "Someone".into());
        assert!(
            bad.to_toml().is_err(),
            "expected conflict when both 'user' and 'user.name' exist"
        );
    }

    #[test]
    fn profile_apply_empty_ok() {
        // Applying an empty profile should fail gracefully only at git repo discovery.
        // We can't guarantee a repo is available in tests, so just ensure method exists
        // and returns Result.
        let p = Profile::default();
        let res = p.apply();
        // Either ok (if tests are run inside a git repo) or an error about not being in
        // a repo.
        if let Err(e) = res {
            // acceptable error
            let msg = e.to_string();
            assert!(
                msg.contains("Failed to open git repository")
                    || msg.contains("could not find repository"),
                "unexpected error: {msg}"
            );
        }
    }
}
