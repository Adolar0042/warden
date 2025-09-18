// Portions of this file are derived from: https://github.com/siketyan/ghr
// Copyright (c) 2022 Naoki Ikeguchi
// Licensed under the MIT License. See LICENSES/MIT-ghr-UPSTREAM.md for details.
//
// Local modifications:
// Copyright (c) 2025 Adolar0042

use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::sync::LazyLock;

use anyhow::{Error, Result, anyhow, bail};
use regex::Regex;
use serde::Deserialize;
use serde_with::DeserializeFromStr;

const GIT_EXTENSION: &str = ".git";
const EXTENSIONS: &[&str] = &[GIT_EXTENSION];

static SSH: LazyLock<Pattern> = LazyLock::new(|| {
    Pattern::from(
        Regex::new(
            r"^(?P<user>[0-9A-Za-z\-]+)@(?P<host>[0-9A-Za-z\.\-]+):(?P<owner>[0-9A-Za-z_\.\-]+)/(?P<repo>[0-9A-Za-z_\.\-]+)$",
        )
        .unwrap(),
    )
    .with_scheme(Scheme::Ssh)
    .with_infer()
});

static HOST_ORG_REPO: LazyLock<Pattern> = LazyLock::new(|| {
    Pattern::from(
        Regex::new(
            r"^(?P<host>[0-9A-Za-z\.\-]+)[:/](?P<owner>[0-9A-Za-z_\.\-]+)/(?P<repo>[0-9A-Za-z_\.\-]+)$",
        )
        .unwrap(),
    )
    .with_infer()
});

static ORG_REPO: LazyLock<Pattern> = LazyLock::new(|| {
    Pattern::from(
        Regex::new(r"^(?P<owner>[0-9A-Za-z_\.\-]+)/(?P<repo>[0-9A-Za-z_\.\-]+)$").unwrap(),
    )
    .with_infer()
});

static REPO: LazyLock<Pattern> = LazyLock::new(|| {
    Pattern::from(Regex::new(r"^(?P<repo>[0-9A-Za-z_\.\-]+)$").unwrap()).with_infer()
});

#[derive(Debug)]
pub struct Match {
    pub vcs: Option<Vcs>,
    pub scheme: Option<Scheme>,
    pub user: Option<String>,
    pub host: Option<Host>,
    pub owner: Option<String>,
    pub repo: String,
    pub raw: Option<String>,
}

/// Describes how to parse and optionally canonicalize repository identifiers or
/// URLs.
///
/// Patterns are tried in order, the first one that matches is used.
/// You can define them in your `profiles.toml` using `[[patterns]]`.
///
/// Minimum requirement:
/// - Your `regex` must capture at least `repo` (using a named group
///   `(?P<repo>...)`).
///
/// Optional named capture groups:
/// - `vcs`: currently only "git" is supported.
/// - `scheme`: either "https" or "ssh".
/// - `user`: SSH username (commonly "git").
/// - `host`: repository host (e.g., "github.com").
/// - `owner`: organization or user (e.g., "torvalds").
///
/// Behavior controls:
/// - `infer = true`: do not store the original string, instead render a canonical form
///   based on captured/defaulted fields (e.g., `https://host/owner/repo.git` or
///   `user@host:owner/repo.git`).
/// - `infer = false` or omitted: keep the original string as the "raw" value
///   unless a `url` template is provided (see below).
///
/// URL template:
/// - If `url` is provided, it is used to render the "raw" string instead of
///   keeping the original input. Supported placeholders: `{{vcs}}`,
///   `{{scheme}}`, `{{user}}`, `{{host}}`, `{{owner}}`, `{{repo}}`.
///
/// TOML examples:
/// ```toml
/// [[patterns]]
/// regex = '^(?P<user>[0-9A-Za-z\\-]+)@(?P<host>[0-9A-Za-z\\.\\-]+):(?
/// P<owner>[0-9A-Za-z_\\.\\-]+)/(?P<repo>[0-9A-Za-z_\\.\\-]+)$' scheme = "ssh"
/// infer = true
///
/// [[patterns]]
/// regex = '^(?P<owner>[0-9A-Za-z_\\.\\-]+)/(?P<repo>[0-9A-Za-z_\\.\\-]+)$'
/// scheme = "https"
/// host = "github.com"
/// infer = true
///
/// [[patterns]]
/// # Render a very specific canonical form instead of inferring
/// regex = '^(?P<scheme>https)://(?P<host>git\\.kernel\\.org)/pub/scm/linux/
/// kernel/git/(?P<owner>.+)/(?P<repo>.+)\\.git' scheme = "https"
/// url = 'https://{{host}}/pub/scm/linux/kernel/git/{{owner}}/{{repo}}.git'
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct Pattern {
    /// Regular expression used to match the input. Must include at least
    /// `(?P<repo>...)`. May optionally include `vcs`, `scheme`, `user`,
    /// `host`, and `owner` named groups.
    #[serde(with = "serde_regex")]
    regex: Regex,
    /// Default VCS when not captured by the regex (currently only "git").
    vcs: Option<Vcs>,
    /// Default URL scheme when not captured by the regex ("https" or "ssh").
    scheme: Option<Scheme>,
    /// Default SSH username when not captured by the regex (e.g., "git").
    user: Option<String>,
    /// Default host when not captured by the regex (e.g., "github.com").
    host: Option<Host>,
    /// Default owner/organization when not captured by the regex.
    owner: Option<String>,
    /// Optional template to render the canonical "raw" URL when `infer` is
    /// false/omitted. Placeholders: `{{vcs}}`, `{{scheme}}`, `{{user}}`,
    /// `{{host}}`, `{{owner}}`, `{{repo}}`.
    url: Option<String>,
    /// Whether to infer a canonical URL (true) or preserve the original string
    /// (false/omitted). If false and `url` is provided, the template is
    /// used to render the "raw" string.
    infer: Option<bool>,
}

impl Pattern {
    #[inline]
    pub const fn with_scheme(mut self, s: Scheme) -> Self {
        self.scheme = Some(s);
        self
    }

    #[inline]
    pub const fn with_infer(mut self) -> Self {
        self.infer = Some(true);
        self
    }

    pub fn matches(&self, s: &str) -> Option<Match> {
        let c = self.regex.captures(s)?;
        let repo_cap = c.name("repo")?;
        let repo = repo_cap.as_str().to_string();

        let mut m = Match {
            vcs: c
                .name("vcs")
                .and_then(|v| Vcs::from_str(v.as_str()).ok())
                .or(self.vcs),
            scheme: c
                .name("scheme")
                .and_then(|v| Scheme::from_str(v.as_str()).ok())
                .or(self.scheme),
            user: c
                .name("user")
                .map(|v| v.as_str().to_string())
                .or_else(|| self.user.clone()),
            host: c
                .name("host")
                .and_then(|v| Host::from_str(v.as_str()).ok())
                .or_else(|| self.host.clone()),
            owner: c
                .name("owner")
                .map(|v| v.as_str().to_string())
                .or_else(|| self.owner.clone()),
            repo,
            raw: None,
        };

        m.raw = if self.infer.unwrap_or(false) {
            None
        } else {
            match &self.url {
                Some(u) => {
                    Some(
                        u.replace("{{vcs}}", &m.vcs.map(|v| v.to_string()).unwrap_or_default())
                            .replace(
                                "{{scheme}}",
                                &m.scheme.map(|s| s.to_string()).unwrap_or_default(),
                            )
                            .replace("{{user}}", &m.user.clone().unwrap_or_default())
                            .replace(
                                "{{host}}",
                                &m.host.clone().map(|h| h.to_string()).unwrap_or_default(),
                            )
                            .replace("{{owner}}", &m.owner.clone().unwrap_or_default())
                            .replace("{{repo}}", &m.repo),
                    )
                },
                None => Some(s.to_string()),
            }
        };

        Some(m)
    }
}

impl From<Regex> for Pattern {
    fn from(value: Regex) -> Self {
        Self {
            regex: value,
            vcs: None,
            scheme: None,
            user: None,
            host: None,
            owner: None,
            url: None,
            infer: None,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Patterns(Vec<Pattern>);

impl Patterns {
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    #[inline]
    pub fn add(&mut self, p: Pattern) {
        self.0.push(p);
    }

    #[inline]
    pub fn with(mut self, p: Pattern) -> Self {
        self.add(p);
        self
    }

    pub fn with_defaults(self) -> Self {
        self.with(SSH.clone())
            .with(HOST_ORG_REPO.clone())
            .with(ORG_REPO.clone())
            .with(REPO.clone())
    }

    pub fn matches(&self, s: &str) -> Option<Match> {
        self.0.iter().find_map(|p| p.matches(s))
    }
}

impl Default for Patterns {
    fn default() -> Self {
        Self::new().with_defaults()
    }
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, DeserializeFromStr)]
pub enum Vcs {
    #[default]
    Git,
}

impl Vcs {
    fn from_url(url: &url::Url) -> Self {
        let url = url.as_str();
        if url.ends_with(GIT_EXTENSION) {
            Self::Git
        } else {
            Self::default()
        }
    }

    const fn extension(self) -> &'static str {
        match self {
            Self::Git => GIT_EXTENSION,
        }
    }
}

impl FromStr for Vcs {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "git" => Self::Git,
            _ => bail!("Unknown VCS found: {}", s),
        })
    }
}

impl Display for Vcs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Git => write!(f, "git"),
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, DeserializeFromStr)]
pub enum Scheme {
    #[default]
    Https,
    Ssh,
}

impl FromStr for Scheme {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "https" => Self::Https,
            "ssh" => Self::Ssh,
            _ => bail!("Unknown URL scheme found: {}", s),
        })
    }
}

impl Display for Scheme {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Https => write!(f, "https"),
            Self::Ssh => write!(f, "ssh"),
        }
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq, DeserializeFromStr)]
pub enum Host {
    #[default]
    GitHub,
    GitLab,
    Codeberg,
    Unknown(String),
}

impl FromStr for Host {
    type Err = Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Infallible> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "github.com" => Self::GitHub,
            "gitlab.com" => Self::GitLab,
            "codeberg.org" => Self::Codeberg,
            _ => Self::Unknown(s.to_string()),
        })
    }
}

impl Display for Host {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitHub => write!(f, "github.com"),
            Self::GitLab => write!(f, "gitlab.com"),
            Self::Codeberg => write!(f, "codeberg.org"),
            Self::Unknown(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq, Clone)]
pub struct Url {
    pub vcs: Vcs,
    pub scheme: Scheme,
    pub user: Option<String>,
    pub host: Host,
    pub owner: String,
    pub repo: String,
    pub raw: Option<String>,
}

impl Url {
    pub fn from_str(s: &str, p: &Patterns, default_owner: Option<&str>) -> Result<Self> {
        Self::from_pattern(s, p, default_owner).or_else(|e| {
            if s.contains("://") {
                Self::from_url(&url::Url::from_str(s)?)
            } else {
                Err(e)
            }
        })
    }

    pub fn from_url(url: &url::Url) -> Result<Self> {
        let mut segments = url
            .path_segments()
            .ok_or_else(|| anyhow!("Could not parse path segments from the URL: {}", url))?;

        let scheme = Scheme::from_str(url.scheme())?;

        Ok(Self {
            vcs: Vcs::from_url(url),
            scheme,
            user: if url.username().is_empty() {
                None
            } else {
                Some(url.username().to_string())
            },
            host: Host::from_str(
                url.host_str()
                    .ok_or_else(|| anyhow!("Could not find hostname from the URL: {}", url))?,
            )?,
            owner: segments
                .next()
                .ok_or_else(|| anyhow!("Could not find owner from the URL: {}", url))?
                .to_string(),
            repo: Self::remove_extensions(
                segments.next().ok_or_else(|| {
                    anyhow!("Could not find repository name from the URL: {}", url)
                })?,
            ),
            raw: match scheme {
                // HTTPS URLs can be used directly on cloning, so we prefer it than inferred one.
                // SSH URLs are not; Git only accepts 'git@github.com:org/repo.git' style.
                Scheme::Https => Some(url.to_string()),
                Scheme::Ssh => None,
            },
        })
    }

    fn from_match(m: Match, default_owner: Option<&str>) -> Option<Self> {
        Some(Self {
            vcs: m.vcs.unwrap_or_default(),
            scheme: m.scheme.unwrap_or_default(),
            user: m.user,
            host: m.host.unwrap_or_default(),
            owner: m
                .owner
                .or_else(|| default_owner.map(std::string::ToString::to_string))?,
            repo: Self::remove_extensions(&m.repo),
            raw: m.raw,
        })
    }

    fn from_pattern(s: &str, p: &Patterns, default_owner: Option<&str>) -> Result<Self> {
        p.matches(s)
            .and_then(|m| Self::from_match(m, default_owner))
            .ok_or_else(|| anyhow!("The input did not match any pattern: {}", s))
    }

    fn remove_extensions(s: &str) -> String {
        let mut out = s;
        for ext in EXTENSIONS {
            if out.ends_with(ext) {
                out = out.trim_end_matches(ext);
            }
        }
        out.to_string()
    }
}

impl Display for Url {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(r) = &self.raw {
            return write!(f, "{r}");
        }

        let authority = self
            .user
            .as_ref()
            .map_or_else(|| self.host.to_string(), |u| format!("{u}@{}", &self.host));

        match self.scheme {
            Scheme::Https => {
                write!(
                    f,
                    "https://{}/{}/{}{}",
                    authority,
                    self.owner,
                    self.repo,
                    self.vcs.extension()
                )
            },
            Scheme::Ssh => {
                write!(
                    f,
                    "{}:{}/{}{}",
                    authority,
                    self.owner,
                    self.repo,
                    self.vcs.extension()
                )
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_from_url_https() {
        let url = url::Url::parse("https://github.com/username/username.github.io.git").unwrap();

        assert_eq!(
            Url {
                vcs: Vcs::Git,
                scheme: Scheme::Https,
                user: None,
                host: Host::GitHub,
                owner: "username".to_string(),
                repo: "username.github.io".to_string(),
                raw: Some("https://github.com/username/username.github.io.git".to_string()),
            },
            Url::from_url(&url).unwrap(),
        );
    }

    #[test]
    fn parse_from_url_ssh() {
        let url = url::Url::parse("ssh://git@github.com/username/username.github.io.git").unwrap();

        assert_eq!(
            Url {
                vcs: Vcs::Git,
                scheme: Scheme::Ssh,
                user: Some("git".to_string()),
                host: Host::GitHub,
                owner: "username".to_string(),
                repo: "username.github.io".to_string(),
                ..Default::default()
            },
            Url::from_url(&url).unwrap(),
        );
    }

    #[test]
    fn parse_from_pattern_repo() {
        assert_eq!(
            Url {
                vcs: Vcs::Git,
                scheme: Scheme::Https,
                user: None,
                host: Host::GitHub,
                owner: "username".to_string(),
                repo: "username.github.io".to_string(),
                ..Default::default()
            },
            Url::from_pattern("username.github.io", &Patterns::default(), Some("username"))
                .unwrap(),
        );
    }

    #[test]
    fn parse_from_pattern_org_repo() {
        assert_eq!(
            Url {
                vcs: Vcs::Git,
                scheme: Scheme::Https,
                user: None,
                host: Host::GitHub,
                owner: "username".to_string(),
                repo: "username.github.io".to_string(),
                ..Default::default()
            },
            Url::from_pattern("username/username.github.io", &Patterns::default(), None).unwrap(),
        );
    }

    #[test]
    fn parse_from_pattern_host_org_repo() {
        assert_eq!(
            Url {
                vcs: Vcs::Git,
                scheme: Scheme::Https,
                user: None,
                host: Host::GitLab,
                owner: "username".to_string(),
                repo: "username.github.io".to_string(),
                ..Default::default()
            },
            Url::from_pattern(
                "gitlab.com:username/username.github.io",
                &Patterns::default(),
                None
            )
            .unwrap(),
        );
    }

    #[test]
    fn parse_from_pattern_ssh() {
        assert_eq!(
            Url {
                vcs: Vcs::Git,
                scheme: Scheme::Ssh,
                user: Some("git".to_string()),
                host: Host::GitHub,
                owner: "username".to_string(),
                repo: "username.github.io".to_string(),
                ..Default::default()
            },
            Url::from_pattern(
                "git@github.com:username/username.github.io.git",
                &Patterns::default(),
                None,
            )
            .unwrap(),
        );
    }

    #[test]
    fn parse_from_custom_pattern() {
        let patterns = Patterns::default().with(
            Pattern::from(
                Regex::new(
                    r"^(?P<scheme>https)://(?P<host>git\.kernel\.org)/pub/scm/linux/kernel/git/(?P<owner>.+)/(?P<repo>.+)\.git",
                )
                .unwrap(),
            )
            .with_scheme(Scheme::Https),
        );

        assert_eq!(
            Url {
                vcs: Vcs::Git,
                scheme: Scheme::Https,
                host: Host::Unknown("git.kernel.org".to_string()),
                owner: "torvalds".to_string(),
                repo: "linux".to_string(),
                raw: Some(
                    "https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git"
                        .to_string(),
                ),
                ..Default::default()
            },
            Url::from_pattern(
                "https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git",
                &patterns,
                None,
            )
            .unwrap(),
        );
    }

    #[test]
    fn to_string_https() {
        assert_eq!(
            "https://github.com/username/username.github.io.git",
            Url {
                vcs: Vcs::Git,
                scheme: Scheme::Https,
                user: None,
                host: Host::GitHub,
                owner: "username".to_string(),
                repo: "username.github.io".to_string(),
                ..Default::default()
            }
            .to_string()
            .as_str(),
        );
    }

    #[test]
    fn to_string_ssh() {
        assert_eq!(
            "git@github.com:username/username.github.io.git",
            Url {
                vcs: Vcs::Git,
                scheme: Scheme::Ssh,
                user: Some("git".to_string()),
                host: Host::GitHub,
                owner: "username".to_string(),
                repo: "username.github.io".to_string(),
                ..Default::default()
            }
            .to_string()
            .as_str(),
        );
    }
}
