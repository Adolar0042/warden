use core::fmt::Display;
use std::cmp::min;
use std::collections::HashMap;
use std::env::consts::OS;

use anyhow::{Context as _, Result};
use chrono::{DateTime, Utc};
use keyring_core::Entry;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use tracing::{info, instrument};
use zeroize::{Zeroize, Zeroizing};

use crate::config::ProviderConfig;
use crate::oauth::refresh_access_token;

struct ProtectedSecret {
    ciphertext: Zeroizing<Vec<u8>>,
    pad: Zeroizing<Vec<u8>>,
}

impl ProtectedSecret {
    fn new(secret: String) -> Result<Self> {
        let mut ciphertext = Zeroizing::new(secret.into_bytes());
        let mut pad = Zeroizing::new(vec![0; ciphertext.len()]);

        // Fill once, then make sure pad is not all zeros
        while !pad.is_empty() && pad.iter().all(|byte| *byte == 0) {
            getrandom::fill(&mut pad).context("Failed to generate secret protection pad")?;
        }

        for (byte, pad_byte) in ciphertext.iter_mut().zip(pad.iter()) {
            *byte ^= pad_byte;
        }

        Ok(Self { ciphertext, pad })
    }

    fn expose<R>(&self, operation: impl FnOnce(&str) -> R) -> R {
        let plaintext = Zeroizing::new(
            self.ciphertext
                .iter()
                .zip(self.pad.iter())
                .map(|(byte, pad_byte)| byte ^ pad_byte)
                .collect::<Vec<_>>(),
        );
        let plaintext =
            str::from_utf8(&plaintext).expect("protected secret must remain valid UTF-8");
        operation(plaintext)
    }
}

impl Serialize for ProtectedSecret {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.expose(|plaintext| serializer.serialize_str(plaintext))
    }
}

impl<'de> Deserialize<'de> for ProtectedSecret {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let plaintext = String::deserialize(deserializer)?;
        Self::new(plaintext).map_err(de::Error::custom)
    }
}

impl Zeroize for ProtectedSecret {
    fn zeroize(&mut self) {
        self.ciphertext.zeroize();
        self.pad.zeroize();
    }
}

#[expect(clippy::struct_field_names, reason = "name is intended")]
#[derive(Serialize, Deserialize)]
pub struct Token {
    access_token: ProtectedSecret,
    refresh_token: Option<ProtectedSecret>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.access_token.expose(|access_token| {
            if access_token.len() <= 4 {
                write!(f, "{}", "*".repeat(access_token.len()))
            } else {
                write!(
                    f,
                    "{}{}",
                    &access_token[0..4],
                    "*".repeat(min(3, access_token.len() - 4))
                )
            }
        })
    }
}

impl Zeroize for Token {
    fn zeroize(&mut self) {
        self.access_token.zeroize();
        if let Some(rt) = &mut self.refresh_token {
            rt.zeroize();
        }
    }
}

impl Token {
    pub fn new(
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Self> {
        Ok(Self {
            access_token: ProtectedSecret::new(access_token)?,
            refresh_token: refresh_token.map(ProtectedSecret::new).transpose()?,
            expires_at,
        })
    }

    pub fn expose_access_token<R, O>(&self, operation: O) -> R
    where
        O: FnOnce(&str) -> R,
    {
        self.access_token.expose(operation)
    }

    pub fn expose_refresh_token<R, O>(&self, operation: O) -> Option<R>
    where
        O: FnOnce(&str) -> R,
    {
        self.refresh_token
            .as_ref()
            .map(|refresh_token| refresh_token.expose(operation))
    }

    pub const fn has_refresh_token(&self) -> bool {
        self.refresh_token.is_some()
    }

    /// Checks if the access token is expired and refreshes it if necessary.
    ///
    /// Side effect: if the token is refreshed, the current instance is updated
    /// with the new token.
    #[instrument(skip(self, provider))]
    pub async fn refresh_if_expired(&mut self, provider: &ProviderConfig) -> Result<()> {
        if self.is_expired() {
            info!("Access token expired, refreshing...");
            let new_token = refresh_access_token(provider, self)
                .await
                .context("Failed to refresh access token")?;
            *self = new_token;
        }
        Ok(())
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|expiry| expiry < Utc::now())
    }

    pub fn pack(&self) -> Result<Zeroizing<String>> {
        serde_json::to_string(self)
            .map(Zeroizing::new)
            .context("Failed to serialize token")
    }

    pub fn from_string(s: &str) -> Result<Self> {
        serde_json::from_str::<Self>(s).context("Failed to deserialize token")
    }
}

fn set_keyring_store() -> Result<()> {
    if keyring_core::get_default_store().is_some() {
        return Ok(());
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    keyring_core::set_default_store(
        zbus_secret_service_keyring_store::Store::new()
            .context("Failed to create keyring store")?,
    );

    #[cfg(target_os = "windows")]
    keyring_core::set_default_store(
        windows_native_keyring_store::Store::new().context("Failed to create keyring store")?,
    );

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    keyring_core::set_default_store(
        apple_native_keyring_store::keychain::Store::new()
            .context("Failed to create keyring store")?,
    );

    #[cfg(not(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly",
        target_os = "windows",
        target_os = "macos",
        target_os = "ios"
    )))]
    bail!("No keyring backend is implemented for platform '{OS}'.");

    Ok(())
}

fn get_entry(credential: &str, host: &str) -> Result<Entry> {
    set_keyring_store().context("Failed to set keyring store")?;
    let entry = match OS {
        "linux" | "freebsd" | "openbsd" | "netbsd" | "dragonfly" => {
            Entry::new_with_modifiers(
                format!("{}:{host}", env!("CARGO_PKG_NAME")).as_str(),
                credential,
                &HashMap::from([(
                    "label",
                    format!("{}:{credential}@{host}", env!("CARGO_PKG_NAME")).as_str(),
                )]),
            )?
        },
        "windows" => {
            Entry::new_with_modifiers(
                format!("{}:{host}", env!("CARGO_PKG_NAME")).as_str(),
                credential,
                &HashMap::from([(
                    "target",
                    format!("{}:{credential}@{host}", env!("CARGO_PKG_NAME")).as_str(),
                )]),
            )?
        },
        _ => {
            Entry::new(
                format!("{}:{host}", env!("CARGO_PKG_NAME")).as_str(),
                credential,
            )?
        },
    };
    Ok(entry)
}

pub fn store_keyring_token(credential: &str, host: &str, token: &Token) -> Result<()> {
    let entry = get_entry(credential, host)?;
    let secret = token.pack()?;
    entry
        .set_password(&secret)
        .context("Failed to set secret in keyring entry")?;

    match OS {
        "linux" | "freebsd" | "openbsd" | "netbsd" | "dragonfly" => {
            // Remove label entry, it is only set in the first place to change
            // the name of the entry in the keyring but unfortunately also shows
            // up in the attributes
            entry.update_attributes(&HashMap::from([(
                "application",
                format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")).as_str(),
            )]))?;
        },
        "windows" => {
            entry.update_attributes(&HashMap::from([(
                "comment",
                format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")).as_str(),
            )]))?;
        },
        _ => {},
    }

    Ok(())
}

pub fn erase_keyring_token(credential: &str, host: &str) -> Result<()> {
    let entry = get_entry(credential, host)?;
    entry.delete_credential()?;
    Ok(())
}

pub fn get_keyring_token(credential: &str, host: &str) -> Result<Token> {
    let entry = get_entry(credential, host)?;
    let secret = Zeroizing::new(
        entry
            .get_password()
            .context("Failed to retrieve token from keyring")?,
    );
    Token::from_string(&secret)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{ProtectedSecret, Token};

    #[test]
    fn protected_secret_only_exposes_plaintext_inside_closure() -> Result<()> {
        let plaintext = "access-token-value";
        let secret = ProtectedSecret::new(plaintext.to_string())?;

        assert_ne!(
            secret.ciphertext.as_slice(),
            plaintext.as_bytes(),
            "ciphertext must not contain the original plaintext"
        );
        secret.expose(|exposed| {
            assert_eq!(exposed, plaintext, "exposed secret must match its input");
        });

        Ok(())
    }

    #[test]
    fn token_serialization_round_trips_protected_secrets() -> Result<()> {
        let token = Token::new(
            "access-token-value".to_string(),
            Some("refresh-token-value".to_string()),
            None,
        )?;
        let packed = token.pack()?;
        let unpacked = Token::from_string(&packed)?;

        unpacked.expose_access_token(|access_token| {
            assert_eq!(
                access_token, "access-token-value",
                "access token must survive serialization"
            );
        });
        let refresh_token = unpacked.expose_refresh_token(|refresh_token| {
            assert_eq!(
                refresh_token, "refresh-token-value",
                "refresh token must survive serialization"
            );
        });
        assert!(refresh_token.is_some(), "refresh token must be present");

        Ok(())
    }
}
