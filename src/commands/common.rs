use colored::Colorize as _;

use crate::config::Hosts;

/// Represents one credential associated with a host
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CredentialPair {
    pub host: String,
    pub credential: String,
}

impl CredentialPair {
    #[inline]
    pub fn new<S: Into<String>>(host: S, credential: S) -> Self {
        Self {
            host: host.into(),
            credential: credential.into(),
        }
    }

    /// Returns credential (host)
    #[inline]
    pub fn label_credential_host(&self) -> String {
        format!("{} ({})", self.credential, self.host)
    }
}

/// Collect all (host, credential) pairs from the `Hosts` config.
///
/// Unsorted; callers can invoke `sort_pairs` for deterministic ordering.
pub fn collect_all_pairs(hosts: &Hosts) -> Vec<CredentialPair> {
    hosts
        .hosts()
        .flat_map(|(host, cfg)| {
            cfg.credentials
                .iter()
                .cloned()
                .map(move |credential| CredentialPair::new(host.to_string(), credential))
        })
        .collect()
}

/// Sort pairs by (host ASC, credential ASC).
pub fn sort_pairs(pairs: &mut [CredentialPair]) {
    pairs.sort_by(|a, b| {
        a.host
            .cmp(&b.host)
            .then_with(|| a.credential.cmp(&b.credential))
    });
}

/// Filter pairs by optional host and/or credential constraints.
///
/// If both filters are `None`, returns the original slice cloned.
/// If a filter removes all pairs, the returned vec is empty
pub fn filter_pairs<'a, T: IntoIterator<Item = &'a CredentialPair>>(
    pairs: T,
    host: Option<&str>,
    credential: Option<&str>,
) -> Vec<CredentialPair> {
    pairs
        .into_iter()
        .filter(|p| host.is_none_or(|h| p.host == h))
        .filter(|p| credential.is_none_or(|c| p.credential == c))
        .cloned()
        .collect()
}

/// Produce a standardized styled error line. *Does not* add a trailing newline.
#[inline]
pub fn styled_error_line<T: AsRef<str>>(msg: T) -> String {
    format!("  {} - {}", "Error".red().bold(), msg.as_ref())
}

/// Turn a slice of `CredentialPair` into "credential (host)" labels
pub fn labels_credential_host(pairs: &[CredentialPair]) -> Vec<String> {
    pairs
        .iter()
        .map(CredentialPair::label_credential_host)
        .collect()
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
    use crate::config::Hosts;
    use crate::config::hosts::HostConfig;

    fn hosts_fixture() -> Hosts {
        Hosts::from_map(HashMap::from([
            (
                "github.com".to_string(),
                HostConfig {
                    active: "alice".into(),
                    credentials: vec!["alice".into(), "bob".into()],
                },
            ),
            (
                "gitlab.com".to_string(),
                HostConfig {
                    active: "carol".into(),
                    credentials: vec!["carol".into()],
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
    fn test_filter_pairs_by_credential() {
        let h = hosts_fixture();
        let all = collect_all_pairs(&h);
        let filtered = filter_pairs(&all, None, Some("carol"));
        assert_eq!(filtered, vec![CredentialPair::new("gitlab.com", "carol")]);
    }

    #[test]
    fn test_filter_pairs_by_host_and_credential() {
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
        let labels = labels_credential_host(&pairs);
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
