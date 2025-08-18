use std::collections::HashMap;

use config::{ConfigError, Source, Value};

/// Git-based configuration source for OAuth provider configuration.
///
/// This source inspects git configuration entries with keys matching the
/// pattern `credential.<base>.oauth<Suffix>`
///
/// Supported (case-insensitive) suffixes:
///   - `ClientId`
///   - `ClientSecret`
///   - `AuthURL`
///   - `TokenURL`
///   - `DeviceAuthURL`
///   - `PreferredFlow`
///   - `Scopes`
///
/// Scopes are split on whitespace or comma. If the parsed list is empty, we
/// emit an explicit empty array (representing `Some(empty)`). If the Scopes key
/// is absent entirely, the provider's `scopes` field will deserialize to
/// `None`.
///
/// `<base>` may include a scheme (`http://` or `https://`). For relative endpoint
/// values (those beginning with `'/'`), the code joins them onto a
/// scheme-bearing base:
///
///   - If `<base>` already includes a scheme, that scheme is kept.
///   - Otherwise `https://` is prefixed when constructing absolute URLs.
///
/// Dots in `<base>` (hostnames like "gitlab.example.com") are preserved because
/// we construct a nested `providers` table instead of emitting flattened
/// dotted keys. That avoids the config crate treating dots as path separators
/// in the provider key itself.
#[derive(Clone, Debug)]
pub struct GitConfigSource {
    mode: GitSourceMode,
}

#[derive(Clone, Copy, Debug)]
enum GitSourceMode {
    GlobalAndSystem,
    RepoLocal,
}

impl GitConfigSource {
    /// system / global / XDG / env Git configuration
    pub const fn global() -> Self {
        Self {
            mode: GitSourceMode::GlobalAndSystem,
        }
    }

    /// Repository-local `.git/config` (discovered from current working dir)
    pub const fn repo() -> Self {
        Self {
            mode: GitSourceMode::RepoLocal,
        }
    }
}

impl Source for GitConfigSource {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new(self.clone())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "This is a single-source config reader, so it has to do a lot of work"
    )]
    fn collect(&self) -> Result<HashMap<String, Value>, ConfigError> {
        use git2::{Config as Git2Config, Repository};

        // get appropriate Git configuration
        let git_cfg_opt = match self.mode {
            GitSourceMode::GlobalAndSystem => Git2Config::open_default().ok(),
            GitSourceMode::RepoLocal => {
                Repository::discover(".")
                    .ok()
                    .and_then(|repo| repo.config().ok())
            },
        };

        let Some(git_cfg) = git_cfg_opt else {
            return Ok(HashMap::new());
        };

        // providers_table maps canonical provider key (host/base) ->
        // Value::Table(fields)
        let mut providers_table: HashMap<String, Value> = HashMap::new();

        if let Ok(mut entries) = git_cfg.entries(Some("credential.*.oauth*")) {
            while let Some(Ok(entry)) = entries.next() {
                let Some(full_key) = entry.name() else {
                    continue;
                };
                let full_key = full_key.to_lowercase();

                if !full_key.starts_with("credential.") {
                    continue;
                }
                let Some(rest) = full_key.strip_prefix("credential.") else {
                    continue;
                };
                let Some(oauth_pos) = rest.find(".oauth") else {
                    continue;
                };

                let raw_base = &rest[..oauth_pos];
                let suffix = &rest[oauth_pos + ".oauth".len()..];
                if suffix.is_empty() {
                    // no suffix -> cannot map
                    continue;
                }

                let trimmed = raw_base.trim_end_matches('/');
                let canonical_base = trimmed
                    .strip_prefix("https://")
                    .or_else(|| trimmed.strip_prefix("http://"))
                    .unwrap_or(trimmed);

                let endpoint_base =
                    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                        trimmed.to_string()
                    } else {
                        format!("https://{canonical_base}")
                    };

                let raw_value = entry.value().unwrap_or_default();

                // resolve relative endpoint values ("/path") against endpoint_base
                let resolve_endpoint = |v: &str| {
                    if v.starts_with('/') {
                        format!("{endpoint_base}{v}")
                    } else {
                        v.to_string()
                    }
                };

                let value_entry = providers_table
                    .entry(canonical_base.to_string())
                    .or_insert_with(|| Value::from(HashMap::<String, Value>::new()));

                let mut table = value_entry
                    .clone()
                    .into_table()
                    .unwrap_or_else(|_| HashMap::<String, Value>::new());

                match suffix {
                    "clientid" => {
                        table.insert("client_id".into(), Value::from(raw_value.to_string()));
                    },
                    "clientsecret" => {
                        table.insert("client_secret".into(), Value::from(raw_value.to_string()));
                    },
                    "authurl" => {
                        table.insert("auth_url".into(), Value::from(resolve_endpoint(raw_value)));
                    },
                    "tokenurl" => {
                        table.insert("token_url".into(), Value::from(resolve_endpoint(raw_value)));
                    },
                    "deviceauthurl" => {
                        table.insert(
                            "device_auth_url".into(),
                            Value::from(resolve_endpoint(raw_value)),
                        );
                    },
                    "preferredflow" => {
                        table.insert("preferred_flow".into(), Value::from(raw_value.to_string()));
                    },
                    "scopes" => {
                        let scopes: Vec<_> = raw_value
                            .split(|c: char| c.is_whitespace() || c == ',')
                            .filter(|s| !s.is_empty())
                            .collect();
                        if scopes.is_empty() {
                            table.insert("scopes".into(), Value::from(Vec::<Value>::new()));
                        } else {
                            let scope_values: Vec<Value> = scopes
                                .iter()
                                .map(|s| Value::from((*s).to_string()))
                                .collect();
                            table.insert("scopes".into(), Value::from(scope_values));
                        }
                    },
                    _ => {
                        // unrecognized suffix, ignore
                    },
                }

                providers_table.insert(canonical_base.to_string(), Value::from(table));
            }
        }

        let oauth_only = git_cfg.get_entry("warden.oauth-only").ok().and_then(|e| {
            e.value().map(|v| {
                let vl = v.to_ascii_lowercase();
                matches!(vl.as_str(), "1" | "true" | "yes" | "on")
            })
        });

        if providers_table.is_empty() && oauth_only.is_none() {
            return Ok(HashMap::new());
        }

        let mut root = HashMap::new();
        if let Some(flag) = oauth_only {
            root.insert("oauth_only".into(), Value::from(flag));
        }
        if !providers_table.is_empty() {
            root.insert("providers".into(), Value::from(providers_table));
        }
        Ok(root)
    }
}

#[cfg(test)]
mod tests {
    // NOTE: These tests are limited to transformation logic assumptions.
    // Full integration tests would require setting up temporary git config
    // files and making libgit2 read them, which is heavier than desired here.

    #[test]
    fn canonical_base_strip_scheme() {
        let raw_base = "https://git.example.com";
        let trimmed = raw_base.trim_end_matches('/');
        let canonical = trimmed
            .strip_prefix("https://")
            .or_else(|| trimmed.strip_prefix("http://"))
            .unwrap_or(trimmed);
        assert_eq!(canonical, "git.example.com");
    }

    #[test]
    fn relative_endpoint_resolution() {
        let endpoint_base = "https://git.example.com".to_string();
        let resolve = |v: &str| {
            if v.starts_with('/') {
                format!("{endpoint_base}{v}")
            } else {
                v.to_string()
            }
        };
        assert_eq!(
            resolve("/oauth/token"),
            "https://git.example.com/oauth/token"
        );
        assert_eq!(resolve("https://override/token"), "https://override/token");
    }
}
