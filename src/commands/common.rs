use colored::Colorize as _;

use crate::config::Hosts;

/// Represents one credential (user) associated with a host.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CredentialPair {
    pub host: String,
    pub user: String,
}

impl CredentialPair {
    #[inline]
    pub fn new<S: Into<String>>(host: S, user: S) -> Self {
        Self {
            host: host.into(),
            user: user.into(),
        }
    }

    /// Returns user (host)
    #[inline]
    pub fn label_user_host(&self) -> String {
        format!("{} ({})", self.user, self.host)
    }
}

/// Collect all (host, user) pairs from the `Hosts` config.
///
/// Unsorted; callers can invoke `sort_pairs` for deterministic ordering.
pub fn collect_all_pairs(hosts: &Hosts) -> Vec<CredentialPair> {
    hosts
        .hosts()
        .flat_map(|(host, cfg)| {
            cfg.users
                .iter()
                .cloned()
                .map(move |user| CredentialPair::new(host.to_string(), user))
        })
        .collect()
}

/// Sort pairs by (host ASC, user ASC).
pub fn sort_pairs(pairs: &mut [CredentialPair]) {
    pairs.sort_by(|a, b| a.host.cmp(&b.host).then_with(|| a.user.cmp(&b.user)));
}

/// Filter pairs by optional host and/or user constraints.
///
/// If both filters are `None`, returns the original slice cloned.
/// If a filter removes all pairs, the returned vec is empty
pub fn filter_pairs<'a, T: IntoIterator<Item = &'a CredentialPair>>(
    pairs: T,
    host: Option<&str>,
    user: Option<&str>,
) -> Vec<CredentialPair> {
    pairs
        .into_iter()
        .filter(|p| host.is_none_or(|h| p.host == h))
        .filter(|p| user.is_none_or(|u| p.user == u))
        .cloned()
        .collect()
}

/// Produce a standardized styled error line. *Does not* add a trailing newline.
#[inline]
pub fn styled_error_line<T: AsRef<str>>(msg: T) -> String {
    format!("  {} - {}", "Error".red().bold(), msg.as_ref())
}

/// Turn a slice of `CredentialPair` into "user (host)" labels
pub fn labels_user_host(pairs: &[CredentialPair]) -> Vec<String> {
    pairs.iter().map(CredentialPair::label_user_host).collect()
}

#[expect(
    clippy::doc_markdown,
    reason = "active_credential isn't a type or function silly!"
)]
/// Turn a slice of `CredentialPair` into "host (active_credential)" labels
pub fn labels_host_active(pairs: &[CredentialPair], hosts: &Hosts) -> Vec<String> {
    pairs
        .iter()
        .map(|p| {
            let active = hosts.get_active_credential(&p.host).unwrap_or_default();
            format!("{} ({})", p.host, active)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::config::{HostConfig, Hosts};

    fn hosts_fixture() -> Hosts {
        Hosts::from_map(HashMap::from([
            (
                "github.com".to_string(),
                HostConfig {
                    active: "alice".into(),
                    users: vec!["alice".into(), "bob".into()],
                },
            ),
            (
                "gitlab.com".to_string(),
                HostConfig {
                    active: "carol".into(),
                    users: vec!["carol".into()],
                },
            ),
        ]))
    }

    #[test]
    fn test_collect_all_pairs() {
        let h = hosts_fixture();
        let mut pairs = collect_all_pairs(&h);
        sort_pairs(&mut pairs);
        assert_eq!(
            pairs,
            vec![
                CredentialPair::new("github.com", "alice"),
                CredentialPair::new("github.com", "bob"),
                CredentialPair::new("gitlab.com", "carol"),
            ]
        );
    }

    #[test]
    fn test_filter_pairs_by_host() {
        let h = hosts_fixture();
        let all = collect_all_pairs(&h);
        let filtered = filter_pairs(&all, Some("github.com"), None);
        assert_eq!(
            filtered,
            vec![
                CredentialPair::new("github.com", "alice"),
                CredentialPair::new("github.com", "bob"),
            ]
        );
    }

    #[test]
    fn test_filter_pairs_by_user() {
        let h = hosts_fixture();
        let all = collect_all_pairs(&h);
        let filtered = filter_pairs(&all, None, Some("carol"));
        assert_eq!(filtered, vec![CredentialPair::new("gitlab.com", "carol")]);
    }

    #[test]
    fn test_filter_pairs_by_host_and_user() {
        let h = hosts_fixture();
        let all = collect_all_pairs(&h);
        let filtered = filter_pairs(&all, Some("github.com"), Some("bob"));
        assert_eq!(filtered, vec![CredentialPair::new("github.com", "bob")]);
    }

    #[test]
    fn test_labels() {
        let h = hosts_fixture();
        let mut pairs = collect_all_pairs(&h);
        sort_pairs(&mut pairs);
        let labels = labels_user_host(&pairs);
        assert!(labels.iter().any(|l| l == "alice (github.com)"));
    }

    #[test]
    fn test_styled_error_line() {
        let line = styled_error_line("Problem happened");
        assert!(
            line.contains("Problem happened"),
            "Styled line missing message: {line}"
        );
    }
}
