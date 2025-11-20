use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use chrono::Utc;
use colored::Colorize as _;
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, Scope,
    TokenResponse as _, TokenUrl,
};
use reqwest::{ClientBuilder, Url, redirect};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::time::{Instant, sleep};
use tracing::{error, instrument};

use crate::config::{OAuthConfig, ProviderConfig};
use crate::keyring::Token;

/// Performs `OAuth2` Authorization Code flow with PKCE to obtain an access
/// token.
#[instrument(skip(provider, config))]
pub async fn exchange_auth_code_pkce(
    provider: &ProviderConfig,
    config: &OAuthConfig,
) -> Result<Token> {
    let (listener, redirect_addr) = bind_listener(config).await?;

    let mut oauth_client = BasicClient::new(ClientId::new(provider.client_id.clone()))
        .set_auth_uri(AuthUrl::new(provider.auth_url.clone())?)
        .set_token_uri(TokenUrl::new(provider.token_url.clone())?)
        .set_redirect_uri(oauth2::RedirectUrl::new(redirect_addr.clone())?);

    if let Some(secret) = &provider.client_secret {
        oauth_client = oauth_client.set_client_secret(ClientSecret::new(secret.clone()));
    }

    let http_client = ClientBuilder::new()
        // following redirects opens the client up to SSRF vulnerabilities
        .redirect(redirect::Policy::none())
        .build()
        .context("Failed to build HTTP client")?;

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let mut auth_req = oauth_client.authorize_url(CsrfToken::new_random);
    if let Some(scopes) = &provider.scopes
        && !scopes.is_empty()
    {
        for s in scopes {
            auth_req = auth_req.add_scope(Scope::new(s.clone()));
        }
    }
    let (authorize_url, csrf_state) = auth_req.set_pkce_challenge(pkce_challenge).url();

    let (code, returned_state) = wait_for_code(&listener, &redirect_addr, &authorize_url).await?;

    if !constant_time_eq::constant_time_eq(
        returned_state.secret().as_bytes(),
        csrf_state.secret().as_bytes(),
    ) {
        return Err(anyhow!("CSRF token mismatch")).context("State validation failed");
    }

    let token_res = oauth_client
        .exchange_code(code)
        .set_pkce_verifier(pkce_verifier)
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
}

/// Bind a local TCP listener on the configured (or ephemeral) port, retrying
/// for up to 5s. Returns the listener and the HTTP redirect base address.
#[instrument(skip(config))]
async fn bind_listener(config: &OAuthConfig) -> Result<(TcpListener, String)> {
    let addr = format!("127.0.0.1:{}", config.port.unwrap_or(0));
    let start = Instant::now();

    let listener = loop {
        match TcpListener::bind(&addr).await {
            Ok(listener) => break listener,
            Err(_) if start.elapsed() < Duration::from_secs(5) => {
                sleep(Duration::from_millis(500)).await;
            },
            Err(err) => {
                error!("Failed to bind TcpListener: {}", err);
                return Err(err).context("TcpListener failed to bind within 5s");
            },
        }
    };

    let redirect_addr = format!("http://{}", listener.local_addr()?);
    Ok((listener, redirect_addr))
}

/// Open the user's browser (best-effort) and wait for the redirect, capturing
/// the authorization code.
///
/// Emits a minimal HTTP response so the user can close the browser tab.
/// Returns the `AuthorizationCode` and the `CsrfToken` returned by the
/// provider.
#[instrument(skip(listener))]
async fn wait_for_code(
    listener: &TcpListener,
    redirect_addr: &str,
    authorize_url: &oauth2::url::Url,
) -> Result<(AuthorizationCode, CsrfToken)> {
    match open::that_detached(authorize_url.to_string()) {
        Ok(()) => {
            eprintln!("Beep Boop! Check your browser for authorization");
        },
        Err(_) => {
            eprintln!(
                "Bzzt! Unable to automatically open your browser.\n Open this URL in your \
                 browser: {}",
                authorize_url.to_string().bold()
            );
        },
    }

    loop {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut reader = BufReader::new(&mut stream);

            let mut request_line = String::new();
            reader.read_line(&mut request_line).await?;

            let redirect_url = request_line.split_whitespace().nth(1).unwrap();
            let url = Url::parse(&format!("{redirect_addr}{redirect_url}"))?;

            let params: HashMap<String, String> = url.query_pairs().into_owned().collect();

            let mut summary = String::new();

            if let Some(err_code) = params.get("error") {
                summary.push_str(err_code);
                if let Some(desc) = params.get("error_description").filter(|s| !s.is_empty()) {
                    summary.push_str(": ");
                    summary.push_str(desc);
                }
                if let Some(uri) = params.get("error_uri").filter(|s| !s.is_empty()) {
                    let _ = write!(summary, " ({uri})");
                }

                write_response(
                    &mut stream,
                    &format!("Something went wrong. You can close this window now. :(\n{summary}"),
                )
                .await?;

                return Err(anyhow!(summary))
                    .context("Authorization failed (provider returned error)");
            }

            let code = params
                .get("code")
                .map(|v| AuthorizationCode::new(v.to_owned()))
                .ok_or_else(|| anyhow!("Missing 'code' parameter in callback URL"))
                .context("Authorization code extraction failed");

            let state = params
                .get("state")
                .map(|v| CsrfToken::new(v.to_owned()))
                .ok_or_else(|| anyhow!("Missing 'state' parameter in callback URL"))
                .context("State parameter extraction failed");

            if let Err(code_err) = code {
                write_response(
                    &mut stream,
                    &format!(
                        "Missing authorization code. You can close this window now. :(\n{code_err}"
                    ),
                )
                .await?;
                return Err(code_err);
            }
            if let Err(state_err) = state {
                write_response(
                    &mut stream,
                    &format!(
                        "Missing state parameter. You can close this window now. :(\n{state_err}"
                    ),
                )
                .await?;
                return Err(state_err);
            }

            let code = code?;
            let state = state?;

            write_response(&mut stream, "You can close this window now. :)").await?;

            break Ok((code, state));
        }
    }
}

#[instrument(skip(stream))]
async fn write_response(stream: &mut (impl AsyncWriteExt + Unpin), body: &str) -> Result<()> {
    let raw = format!(
        "HTTP/1.1 200 OK\r\ncontent-length: {len}\r\ncontent-type: text/plain; \
         charset=utf-8\r\n\r\n{body}",
        len = body.len(),
        body = body
    );
    stream
        .write_all(raw.as_bytes())
        .await
        .context("Failed to write HTTP response")?;
    Ok(())
}
