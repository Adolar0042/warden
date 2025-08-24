use core::fmt::Display;
use std::cmp::min;
use std::collections::HashMap;

use anyhow::{Context as _, Result};
use chrono::{DateTime, Utc};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::config::ProviderConfig;
use crate::oauth::refresh_access_token;

#[expect(clippy::struct_field_names, reason = "name is intended")]
#[derive(Serialize, Deserialize, Clone)]
pub struct Token {
    access_token: String,
    refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.access_token.len() > 4 {
            write!(
                f,
                "{}{}",
                &self.access_token[0..4],
                "*".repeat(min(3, self.access_token.len() - 4))
            )
        } else {
            write!(f, "{}", "*".repeat(self.access_token.len()))
        }
    }
}

impl Token {
    pub const fn new(
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            access_token,
            refresh_token,
            expires_at,
        }
    }

    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// Checks if the access token is expired and refreshes it if necessary.
    /// Returns the access token if it is valid, or refreshes it and returns the
    /// new token.
    ///
    /// Side effect: if the token is refreshed, the current instance is updated
    /// with the new token.
    #[instrument(skip(self, provider))]
    pub async fn access_token_checked(&mut self, provider: &ProviderConfig) -> Result<&str> {
        if self.expires_at.is_some_and(|dt| dt < Utc::now()) {
            info!("Access token expired, refreshing...");
            let new_token = refresh_access_token(provider, self)
                .await
                .context("Failed to refresh access token")?;
            *self = new_token;
        }
        Ok(&self.access_token)
    }

    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }

    pub fn pack(&self) -> String {
        serde_json::to_string(self)
            .context("Failed to serialize token for storage in keyring")
            .unwrap()
    }

    pub fn from_string(s: &str) -> Result<Self> {
        serde_json::from_str::<Self>(s).context("Failed to parse token")
    }
}

fn get_entry(credential: &str, host: &str) -> Result<Entry> {
    #[cfg(not(target_os = "windows"))]
    let entry = Entry::new(
        format!("{}:{host}", env!("CARGO_PKG_NAME")).as_str(),
        credential,
    )?;
    #[cfg(target_os = "windows")]
    let entry = Entry::new_with_target(
        format!("{}:{credential}@{host}", env!("CARGO_PKG_NAME")).as_str(),
        format!("{}:{host}", env!("CARGO_PKG_NAME")).as_str(),
        credential,
    )?;
    Ok(entry)
}

pub fn store_keyring_token(credential: &str, host: &str, token: &Token) -> Result<()> {
    let entry = get_entry(credential, host)?;
    entry.set_password(&token.pack())?;
    #[cfg(target_os = "linux")]
    entry.update_attributes(&HashMap::from([
        (
            "label",
            format!("{}:{credential}@{host}", env!("CARGO_PKG_NAME")).as_str(),
        ),
        (
            "application",
            format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")).as_str(),
        ),
    ]))?;
    #[cfg(target_os = "windows")]
    entry.update_attributes(&HashMap::from([(
        "comment",
        format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")).as_str(),
    )]))?;
    Ok(())
}

pub fn erase_keyring_token(credential: &str, host: &str) -> Result<()> {
    let entry = get_entry(credential, host)?;
    entry.delete_credential()?;
    Ok(())
}

pub fn get_keyring_token(credential: &str, host: &str) -> Result<Token> {
    let entry = get_entry(credential, host)?;
    let secret = entry
        .get_password()
        .context("Failed to retrieve token from keyring.")?;
    Token::from_string(&secret)
}
