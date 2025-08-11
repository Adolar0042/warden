use std::collections::HashMap;
use std::io::{self, BufRead as _};
use std::path::PathBuf;
use std::sync::LazyLock;

use anyhow::{Context as _, Result};
use dialoguer::console::style;
use dialoguer::theme::ColorfulTheme;
use tracing::{info, instrument};

pub static THEME: LazyLock<ColorfulTheme> = LazyLock::new(|| {
    ColorfulTheme {
        prompt_prefix: style(String::new()).for_stderr(),
        prompt_suffix: style(String::new()).for_stderr(),
        success_prefix: style(String::new()).for_stderr(),
        success_suffix: style(String::new()).for_stderr(),
        error_prefix: style(String::new()).for_stderr(),
        active_item_prefix: style(">".to_string()).for_stderr().magenta(),
        inactive_item_prefix: style(" ".to_string()).for_stderr(),
        ..ColorfulTheme::default()
    }
});

/// Represents the fields Git sends to a credential helper.
#[derive(Debug)]
pub struct CredentialRequest {
    pub _protocol: String,
    pub host: String,
    pub _path: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
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
    info!("{:#?}", &map);

    Ok(CredentialRequest {
        _protocol: map
            .get("protocol")
            .cloned()
            .context("Missing 'protocol' field")?,
        host: map.get("host").cloned().context("Missing 'host' field")?,
        _path: map.get("path").cloned(),
        username: map.get("username").cloned(),
        password: map.get("password").cloned(),
    })
}

#[cfg(not(target_os = "linux"))]
#[instrument]
pub fn config_dir() -> Result<PathBuf> {
    dirs::home_dir()
        .context("Failed to get home directory")
        .map(|dir| dir.join(".config").join(env!("CARGO_PKG_NAME")))
}

#[cfg(target_os = "linux")]
#[instrument]
pub fn config_dir() -> Result<PathBuf> {
    dirs::config_dir()
        .context("Failed to get config directory")
        .map(|dir| dir.join(env!("CARGO_PKG_NAME")))
}
