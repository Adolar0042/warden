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

/// Prints the token in the format expected by git
pub fn print_token(token: &Token, username: &str) {
    println!("username={username}");
    token.expose_access_token(|access_token| println!("password={access_token}"));
    if let Some(timestamp) = token.expires_at {
        println!("password_expiry_utc={}", timestamp.timestamp());
    }
    let _ = token.expose_refresh_token(|refresh_token| {
        println!("oauth_refresh_token={refresh_token}");
    });
}

/// Prints the token in the format expected by Git, refreshing the token when
/// needed and possible
pub async fn print_token_checked(
    token: &mut Token,
    username: &str,
    provider: &ProviderConfig,
) -> Result<()> {
    token
        .refresh_if_expired(provider)
        .await
        .context("Failed to get access token")?;
    print_token(token, username);
    Ok(())
}
