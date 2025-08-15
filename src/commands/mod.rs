use anyhow::{Context as _, Result};

use crate::config::ProviderConfig;
use crate::keyring::Token;

pub mod apply;
pub mod common;
pub mod erase;
pub mod get;
pub mod list;
pub mod login;
pub mod logout;
pub mod refresh;
pub mod show;
pub mod status;
pub mod store;
pub mod switch;

fn emit_token_lines(username: &str, token: &Token) {
    println!("username={username}");
    println!("password={}", token.access_token());
    if let Some(timestamp) = token.expires_at {
        println!("password_expiry_utc={}", timestamp.timestamp());
    }
    if let Some(refresh_token) = token.refresh_token() {
        println!("oauth_refresh_token={refresh_token}");
    }
}

/// Prints the token in the format expected by Git.
pub fn print_token(token: &Token, username: &str) {
    emit_token_lines(username, token);
}

/// Prints the token in the format expected by Git, refreshing the token when
/// needed and possible.
pub async fn print_token_checked(
    token: &mut Token,
    username: &str,
    provider: &ProviderConfig,
) -> Result<()> {
    let _ = token
        .access_token_checked(provider)
        .await
        .context("Failed to get or refresh access token")?;
    emit_token_lines(username, token);
    Ok(())
}
