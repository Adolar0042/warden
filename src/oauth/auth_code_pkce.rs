use std::time::Duration;

use anyhow::{Context as _, Result};
use colored::Colorize as _;
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, Scope,
    TokenResponse as _, TokenUrl,
};
use reqwest::Url;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::TcpListener;
use tokio::time::{Instant, sleep};
use tracing::{error, instrument};

use crate::config::{OAuthConfig, ProviderConfig};

/// Performs `OAuth2` Authorization Code flow with PKCE to obtain an access
/// token.
#[instrument(skip(provider, config))]
pub async fn exchange_auth_code_pkce(
    provider: &ProviderConfig,
    config: &OAuthConfig,
) -> Result<String> {
    let mut addr = format!("127.0.0.1:{}", config.port.unwrap_or(0));
    let start = Instant::now();
    let listener = loop {
        match TcpListener::bind(&addr).await {
            Ok(listener) => break listener,
            Err(_) if start.elapsed() < Duration::from_secs(5) => {
                sleep(Duration::from_millis(500)).await;
            },
            Err(err) => {
                error!("Failed to bind TcpListener: {}", err);
                return Err(err).context("TcpListener didn't bind within 5s");
            },
        }
    };
    addr = format!("http://{}", listener.local_addr()?);

    let client = BasicClient::new(ClientId::new(provider.client_id.clone()))
        .set_client_secret(ClientSecret::new(provider.client_secret.clone()))
        .set_auth_uri(AuthUrl::new(provider.auth_url.clone())?)
        .set_token_uri(TokenUrl::new(provider.token_url.clone())?)
        .set_redirect_uri(oauth2::RedirectUrl::new(addr.clone())?);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let http_client = reqwest::ClientBuilder::new()
        // following redirects opens the client up to SSRF vulnerabilities
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    // generate the authorization URL to which we'll redirect the user
    let (authorize_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(provider.scopes.join(" ")))
        .set_pkce_challenge(pkce_challenge)
        .url();

    let (code, state) = {
        match open::that(authorize_url.to_string()) {
            Ok(()) => {
                eprintln!("Check your browser for authorization.");
            },
            Err(_) => {
                eprintln!(
                    "Unable to automatically open your browser.\nOpen this URL in your browser: {}",
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
                let url = Url::parse(&(addr + redirect_url))?;

                let code = url
                    .query_pairs()
                    .find(|(key, _)| key == "code")
                    .map(|(_, code)| AuthorizationCode::new(code.into_owned()))
                    .expect("Missing 'code' parameter in redirect URL");

                let state = url
                    .query_pairs()
                    .find(|(key, _)| key == "state")
                    .map(|(_, state)| CsrfToken::new(state.into_owned()))
                    .expect("Missing 'state' parameter in redirect URL");

                let message = "You can close this window now. :)";
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{}",
                    message.len(),
                    message
                );
                stream.write_all(response.as_bytes()).await?;

                break (code, state);
            }
        }
    };

    // check if the CSRF state matches
    assert!(
        // use constant-time comparison to prevent timing attacks
        // unlikely to be a problem here, but good practice
        constant_time_eq::constant_time_eq(
            state.secret().as_bytes(),
            csrf_state.secret().as_bytes(),
        ),
        "CSRF token mismatch"
    );

    // exchange code for a token
    let token_res = client
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
    let access_token = token.access_token().secret().clone();
    Ok(access_token)
}
