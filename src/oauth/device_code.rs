use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use colored::Colorize as _;
use oauth2::{
    AuthType, AuthUrl, ClientId, ClientSecret, DeviceAuthorizationResponse, DeviceAuthorizationUrl,
    ExtraDeviceAuthorizationFields, RequestTokenError, Scope, TokenResponse as _, TokenUrl,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, instrument};

use crate::config::{OAuthConfig, ProviderConfig};

#[derive(Debug, Serialize, Deserialize)]
struct StoringFields(HashMap<String, Value>);

impl ExtraDeviceAuthorizationFields for StoringFields {}
type StoringDeviceAuthorizationResponse = DeviceAuthorizationResponse<StoringFields>;

#[instrument(skip(provider, _config))]
pub async fn exchange_device_code(
    provider: &ProviderConfig,
    _config: &OAuthConfig,
) -> Result<String> {
    let auth_url =
        AuthUrl::new(provider.auth_url.clone()).expect("Invalid authorization endpoint URL");
    let token_url = TokenUrl::new(provider.token_url.clone()).expect("Invalid token endpoint URL");
    let device_auth_url = DeviceAuthorizationUrl::new(
        provider
            .device_auth_url
            .clone()
            .expect("Missing device_auth_url in config"),
    )
    .expect("Invalid device authorization endpoint URL");

    let device_client = oauth2::basic::BasicClient::new(ClientId::new(provider.client_id.clone()))
        .set_client_secret(ClientSecret::new(provider.client_secret.clone()))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url.clone())
        .set_device_authorization_url(device_auth_url)
        .set_auth_type(AuthType::RequestBody);

    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    let details: StoringDeviceAuthorizationResponse = device_client
        .exchange_device_code()
        .add_scope(Scope::new(provider.scopes.join(" ")))
        .request_async(&http_client)
        .await
        .context("Failed to request device authorization codes")?;

    let _ = open::that(details.verification_uri().to_string());

    eprintln!(
        "Beep Boop! Open this URL in your browser\n{}\nand enter the code {}\n",
        details.verification_uri().bold(),
        details.user_code().secret().bold()
    );

    loop {
        let token = device_client
            .exchange_device_access_token(&details)
            .request_async(
                &http_client,
                tokio::time::sleep,
                Duration::from_secs(5).into(),
            )
            .await;
        match token {
            Ok(token) => return Ok(token.access_token().clone().into_secret()),
            Err(RequestTokenError::Parse(_, serde_error)) => {
                if String::from_utf8(serde_error)?.contains("authorization_pending") {
                    // we got a github!
                    // break and enter the weird loop for non-oauth2 compliant servers
                    info!("Git server is not following the oauth2 spec.");
                    break;
                }
            },
            _ => {},
        }
    }

    // weird custom implementation for github
    loop {
        let res = http_client
            .post(token_url.as_str())
            .header("Accept", "application/json")
            .form(&[
                ("client_id", provider.client_id.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", details.device_code().secret()),
            ])
            .send()
            .await
            .context("Failed to request access token via device flow")?;

        let json: Value = res.json().await.context("Failed to parse token response")?;

        if let Some(err) = json.get("error").and_then(Value::as_str) {
            match err {
                "authorization_pending" => {
                    tokio::time::sleep(details.interval()).await;
                    continue;
                },
                "slow_down" => {
                    tokio::time::sleep(details.interval() + Duration::from_secs(5)).await;
                    continue;
                },
                other => bail!("Device flow error: {} - {:?}", other, json),
            }
        }

        let access = json
            .get("access_token")
            .and_then(Value::as_str)
            .context("Missing access_token in response")?
            .to_string();

        return Ok(access);
    }
}
