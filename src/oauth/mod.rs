pub mod auth_code_pkce;
pub mod device_code;
use anyhow::{Result, bail};
use chrono::Utc;
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, RefreshToken, TokenResponse as _, TokenUrl};
use reqwest::{ClientBuilder, redirect};
use tracing::{error, instrument};

use crate::config::{OAuthConfig, ProviderConfig};
use crate::keyring::Token;

/// Selects and executes the OAuth flow based on provider settings.
#[instrument(skip(provider, config))]
pub async fn get_access_token(
    provider: &ProviderConfig,
    config: &OAuthConfig,
    force_device: bool,
) -> Result<Token> {
    if force_device {
        if provider.device_auth_url.is_none() {
            bail!("Device code flow is not supported for this provider.");
        }
        return device_code::exchange_device_code(provider, config).await;
    }
    match provider.preferred_flow.as_deref() {
        Some("device") => device_code::exchange_device_code(provider, config).await,
        Some("authcode") => auth_code_pkce::exchange_auth_code_pkce(provider, config).await,
        _ => {
            if provider.device_auth_url.is_some() {
                // Try device flow first, fall back to auth code
                match device_code::exchange_device_code(provider, config).await {
                    Ok(secret) => Ok(secret),
                    Err(_) => auth_code_pkce::exchange_auth_code_pkce(provider, config).await,
                }
            } else {
                auth_code_pkce::exchange_auth_code_pkce(provider, config).await
            }
        },
    }
}

/// Refreshes the access token using the refresh token.
#[instrument(skip(provider, token))]
pub async fn refresh_access_token(provider: &ProviderConfig, token: &Token) -> Result<Token> {
    if let Some(refresh_token) = &token.refresh_token() {
        let mut client = BasicClient::new(ClientId::new(provider.client_id.clone()))
            .set_auth_uri(AuthUrl::new(provider.auth_url.clone())?)
            .set_token_uri(TokenUrl::new(provider.token_url.clone())?);
        if let Some(secret) = &provider.client_secret {
            client = client.set_client_secret(ClientSecret::new(secret.clone()));
        }

        let http_client = ClientBuilder::new()
            .redirect(redirect::Policy::none())
            .build()
            .expect("Client should build");

        let token_res = client
            .exchange_refresh_token(&RefreshToken::new((*refresh_token).to_string()))
            .request_async(&http_client)
            .await;
        let token = match token_res {
            Ok(token) => token,
            Err(err) => {
                error!("Failed to exchange code: {}", err);
                return Err(err.into());
            },
        };
        let expires_at = token.expires_in().map(|d| Utc::now() + d);
        let token = Token::new(
            token.access_token().secret().clone(),
            token.refresh_token().map(|rt| rt.secret().clone()),
            expires_at,
        );
        Ok(token)
    } else {
        bail!("No refresh token available")
    }
}
