pub mod auth_code_pkce;
pub mod device_code;
use anyhow::Result;

use crate::config::{OAuthConfig, ProviderConfig};

/// Selects and executes the OAuth flow based on provider settings.
pub async fn get_access_token(provider: &ProviderConfig, config: &OAuthConfig) -> Result<String> {
    match provider.preferred_flow.as_deref() {
        Some("device") => device_code::exchange_device_code(provider, config).await,
        Some("authcode") => auth_code_pkce::exchange_auth_code_pkce(provider, config).await,
        _ => {
            if provider.device_auth_url.is_some() {
                // Try device flow first, fall back to auth code
                match device_code::exchange_device_code(provider, config).await {
                    Ok(token) => Ok(token),
                    Err(_) => auth_code_pkce::exchange_auth_code_pkce(provider, config).await,
                }
            } else {
                auth_code_pkce::exchange_auth_code_pkce(provider, config).await
            }
        },
    }
}
