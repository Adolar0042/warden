use std::collections::HashMap;
use std::string;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use chrono::Utc;
use colored::Colorize as _;
use oauth2::basic::BasicClient;
use oauth2::{
    AuthType, AuthUrl, ClientId, ClientSecret, DeviceAuthorizationResponse, DeviceAuthorizationUrl,
    ExtraDeviceAuthorizationFields, RequestTokenError, Scope, TokenResponse as _, TokenUrl,
};
use qr2term::matrix::Matrix;
use qr2term::render::Renderer;
use qrcode::{Color, EcLevel, QrCode};
use reqwest::{ClientBuilder, redirect};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::sleep;
use tracing::{info, instrument};

use crate::config::ProviderConfig;
use crate::keyring::Token;

#[derive(Debug, Serialize, Deserialize)]
struct StoringFields(HashMap<String, Value>);

impl ExtraDeviceAuthorizationFields for StoringFields {}
type StoringDeviceAuthorizationResponse = DeviceAuthorizationResponse<StoringFields>;

#[expect(
    clippy::too_many_lines,
    reason = "function is long but necessary for device code flow"
)]
#[instrument(skip(provider))]
pub async fn exchange_device_code(provider: &ProviderConfig) -> Result<Token> {
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

    let mut device_client = BasicClient::new(ClientId::new(provider.client_id.clone()))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url.clone())
        .set_device_authorization_url(device_auth_url)
        .set_auth_type(AuthType::RequestBody);
    if let Some(secret) = &provider.client_secret {
        device_client = device_client.set_client_secret(ClientSecret::new(secret.clone()));
    }

    let http_client = ClientBuilder::new()
        .redirect(redirect::Policy::none())
        .build()
        .expect("Client should build");

    let mut device_auth_req = device_client.exchange_device_code();
    if let Some(scopes) = &provider.scopes
        && !scopes.is_empty()
    {
        for s in scopes {
            device_auth_req = device_auth_req.add_scope(Scope::new(s.clone()));
        }
    }
    let details: StoringDeviceAuthorizationResponse = device_auth_req
        .request_async(&http_client)
        .await
        .context("Failed to request device authorization codes")?;

    if let Some(uri_complete) = details.verification_uri_complete() {
        let _ = open::that_detached(uri_complete.secret());
        let mut qr_code: Option<String> = None;

        if let Ok(qr) = QrCode::with_error_correction_level(uri_complete.secret(), EcLevel::L) {
            let mut matrix = Matrix::new(qr.to_colors());
            matrix.surround(2, Color::Light);
            let mut buf = Vec::new();
            if matches!(Renderer::default().render(&matrix, &mut buf), Ok(()))
                && let Ok(s) = String::from_utf8(buf)
            {
                qr_code = Some(s);
            }
        }

        eprintln!(
            "Beep Boop! Open this URL in your browser{}",
            if qr_code.is_some() {
                " or scan the QR code below"
            } else {
                ""
            }
        );
        eprintln!("{}", uri_complete.secret().bold());
        if let Some(code) = qr_code {
            eprintln!("{code}");
        }
    } else {
        let _ = open::that_detached(details.verification_uri().to_string());

        eprintln!(
            "Beep Boop! Open this URL in your browser\n{}\nand enter the code {}",
            details.verification_uri().bold(),
            details.user_code().secret().bold()
        );
    }

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
            Ok(token) => {
                let expires_at = token.expires_in().map(|d| Utc::now() + d);
                let token = Token::new(
                    token.access_token().secret().clone(),
                    token.refresh_token().map(|s| s.secret().clone()),
                    expires_at,
                );
                return Ok(token);
            },
            Err(RequestTokenError::Parse(_, serde_error)) => {
                if String::from_utf8(serde_error)?.contains("authorization_pending") {
                    // we got a github!
                    // break and enter the weird loop for non-oauth2 compliant servers
                    info!("Provider is not following the oauth2 spec");
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
                    sleep(details.interval()).await;
                    continue;
                },
                "slow_down" => {
                    sleep(details.interval() + Duration::from_secs(5)).await;
                    continue;
                },
                other => {
                    let mut summary = String::new();
                    summary.push_str(other);
                    if let Some(desc) = json.get("error_description").and_then(Value::as_str) {
                        summary.push_str(": ");
                        summary.push_str(desc);
                    }
                    if let Some(uri) = json.get("error_uri").and_then(Value::as_str) {
                        summary.push_str(" (");
                        summary.push_str(uri);
                        summary.push(')');
                    }
                    return Err(anyhow!("{json:?}"))
                        .context(summary)
                        .context("Failed to get access token via device flow");
                },
            }
        }

        let access_token = json
            .get("access_token")
            .and_then(Value::as_str)
            .context("Missing access_token in response")?
            .to_string();
        let refresh_token = json
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(string::ToString::to_string);
        let expires_in = json
            .get("expires_in")
            .and_then(Value::as_u64)
            .map(Duration::from_secs);
        let expires_at = expires_in.map(|d| Utc::now() + d);
        let token = Token::new(access_token, refresh_token, expires_at);

        return Ok(token);
    }
}
