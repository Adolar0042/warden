// Portions of this file are derived from: https://github.com/siketyan/ghr
// Copyright (c) 2022 Naoki Ikeguchi
// Licensed under the MIT License. See LICENSES/MIT-ghr-UPSTREAM.md for details.
//
// Local modifications:
// Copyright (c) 2025 Adolar0042

use serde::Deserialize;

use crate::profile::url::Url;

#[derive(Clone, Debug, Deserialize)]
pub struct ProfileRef {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Rule {
    pub profile: ProfileRef,
    pub host: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
}

impl Rule {
    pub fn matches(&self, url: &Url) -> bool {
        let url_host = format!("{}", url.host);
        let host_match = self.host.as_deref().is_none_or(|h| h == url_host);
        let owner_match = self.owner.as_deref().is_none_or(|o| o == url.owner);
        let repo_match = self.repo.as_deref().is_none_or(|r| r == url.repo);
        host_match && owner_match && repo_match
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Rules(Vec<Rule>);

impl Rules {
    pub fn resolve(&self, url: &Url) -> Option<&Rule> {
        self.0.iter().find(|rule| rule.matches(url))
    }
}
