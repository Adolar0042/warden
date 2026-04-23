use std::collections::HashMap;
use std::env::consts::FAMILY;
use std::fmt::Display;
use std::io::{self, BufRead as _, stderr};
use std::path::PathBuf;
use std::process::exit;

use anyhow::{Context as _, Result, anyhow};
use chrono::{DateTime, Utc};
use crossterm::cursor::Show;
use crossterm::execute;
use dialoguer::FuzzySelect;
use tracing::{error, info, instrument};

use crate::theme::InputTheme;

pub fn select_index<S: Into<String>, T: AsRef<str> + Display>(
    items: &[T],
    prompt: S,
) -> Result<usize> {
    let _ = ctrlc::set_handler(|| {
        let _ = execute!(stderr(), Show);
        exit(130);
    });
    let sel = FuzzySelect::with_theme(&InputTheme::default())
        .items(items)
        .with_prompt(prompt)
        .default(0)
        .interact_opt()
        .context("Failed to select")?;
    #[expect(clippy::option_if_let_else, reason = "match is more readable here")]
    match sel {
        Some(index) => Ok(index),
        None => {
            exit(130);
        },
    }
}

/// Represents the fields Git sends to a credential helper.
#[derive(Debug)]
pub struct CredentialRequest {
    pub _protocol: String,
    pub host: String,
    pub _path: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub password_expiry_utc: Option<DateTime<Utc>>,
    pub oauth_refresh_token: Option<String>,
}

/// Parses Git's credential helper input from stdin (key=value pairs).
#[instrument]
pub fn parse_credential_request() -> Result<CredentialRequest> {
    let stdin = io::stdin();
    let lines = stdin.lock().lines();
    let mut map = HashMap::new();

    for line_res in lines {
        let line = line_res?;
        if line.is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.to_string(), value.to_string());
        }
    }
    info!(
        "{:#?}",
        &map.clone()
            .iter()
            .map(|(k, v)| {
                if k == "password" || k == "oauth_refresh_token" {
                    (k.clone(), "[REDACTED]".to_string())
                } else {
                    (k.clone(), v.clone())
                }
            })
            .collect::<HashMap<_, _>>()
    );

    let password_expiry_utc = map
        .get("password_expiry_utc")
        .map(|s| {
            let ts = s
                .trim()
                .parse::<u64>()
                .context("Invalid 'password_expiry_utc'")?;

            DateTime::from_timestamp(
                ts.try_into().context("password_expiry_utc out of range")?,
                0,
            )
            .ok_or_else(|| {
                error!("Invalid 'password_expiry_utc' timestamp: {ts}");
                anyhow!("Invalid 'password_expiry_utc' timestamp: {ts}")
            })
        })
        .transpose()?;

    Ok(CredentialRequest {
        _protocol: map
            .get("protocol")
            .cloned()
            .context("Missing 'protocol' field")?,
        host: map.get("host").cloned().context("Missing 'host' field")?,
        _path: map.get("path").cloned(),
        username: map.get("username").cloned(),
        password: map.get("password").cloned(),
        password_expiry_utc,
        oauth_refresh_token: map.get("oauth_refresh_token").cloned(),
    })
}

#[instrument]
pub fn config_dir() -> Result<PathBuf> {
    match FAMILY {
        "unix" => {
            dirs::config_dir()
                .context("Failed to get config directory")
                .map(|dir| dir.join(env!("CARGO_PKG_NAME")))
        },
        _ => {
            dirs::home_dir()
                .context("Failed to get home directory")
                .map(|dir| dir.join(".config").join(env!("CARGO_PKG_NAME")))
        },
    }
}
