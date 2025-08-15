use core::fmt::Display;
use std::cmp::min;
use std::collections::HashMap;

use anyhow::{Context as _, Result};
use keyring::Entry;

pub struct Token {
    secret: String,
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.secret.len() > 4 {
            write!(
                f,
                "{}{}",
                &self.secret[0..4],
                "*".repeat(min(3, self.secret.len() - 4))
            )
        } else {
            write!(f, "{}", "*".repeat(self.secret.len()))
        }
    }
}

impl Token {
    pub const fn new(secret: String) -> Self {
        Self { secret }
    }

    pub fn secret(&self) -> &str {
        &self.secret
    }
}

fn get_entry(user: &str, host: &str) -> Result<Entry> {
    #[cfg(not(target_os = "windows"))]
    let entry = Entry::new(format!("{}:{host}", env!("CARGO_PKG_NAME")).as_str(), user)?;
    #[cfg(target_os = "windows")]
    let entry = Entry::new_with_target(
        format!("{}:{user}@{host}", env!("CARGO_PKG_NAME")).as_str(),
        format!("{}:{host}", env!("CARGO_PKG_NAME")).as_str(),
        user,
    )?;
    Ok(entry)
}

pub fn store_keyring_token(user: &str, host: &str, token: &str) -> Result<()> {
    let entry = get_entry(user, host)?;
    entry.set_password(token)?;
    #[cfg(target_os = "linux")]
    entry.update_attributes(&HashMap::from([
        (
            "label",
            format!("{}:{user}@{host}", env!("CARGO_PKG_NAME")).as_str(),
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

pub fn erase_keyring_token(user: &str, host: &str) -> Result<()> {
    let entry = get_entry(user, host)?;
    entry.delete_credential()?;
    Ok(())
}

pub fn get_keyring_token(user: &str, host: &str) -> Result<Token> {
    let entry = get_entry(user, host)?;
    let secret = entry
        .get_password()
        .context("Failed to retrieve token from keyring.")?;
    Ok(Token::new(secret))
}
