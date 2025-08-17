#![cfg_attr(doc, doc = include_str!("../README.md"))]

use anyhow::{Context as _, Result};
use clap::{CommandFactory as _, Parser, Subcommand};
use clap_complete::{Shell, generate};
use tracing::instrument;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, fmt, registry};

use crate::config::{Hosts, OAuthConfig, ProfileConfig};
use crate::profile::rule::ProfileRef;

mod commands;
mod config;
mod keyring;
mod oauth;
mod profile;
mod utils;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Use OAuth device flow or fail
    #[clap(short, long, global = true)]
    device: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Retrieve credentials
    #[command(hide = true)]
    Get,
    /// Store credentials
    #[command(hide = true)]
    Store,
    /// Erase credentials
    #[command(hide = true)]
    Erase,
    /// Lists all configured profiles.
    List {
        /// Shows only the name of the profiles
        #[clap(short, long)]
        short: bool,
    },
    /// Shows a profile in TOML format.
    Show { profile: String },
    /// Apply a profile.
    Apply { profile: Option<String> },
    /// Login to a provider and store the credentials.
    Login,
    /// Logout from a provider and erase the credentials.
    Logout {
        /// The hostname to logout from
        #[clap(short, long)]
        hostname: Option<String>,
        /// The credential name to logout from
        #[clap(short, long)]
        name: Option<String>,
    },
    /// Refresh credentials for a provider.
    Refresh {
        /// The hostname to refresh credentials for
        #[clap(short, long)]
        hostname: Option<String>,
        /// The credential name to refresh
        #[clap(short, long)]
        name: Option<String>,
    },
    /// Switch between credentials.
    Switch {
        /// The hostname to switch credentials for
        #[clap(short, long)]
        hostname: Option<String>,
        /// The credential name to switch to
        #[clap(short, long)]
        name: Option<String>,
    },
    /// Show the current status of the credentials.
    Status,
    /// Generate shell completions for the given shell.
    Completions {
        #[clap(value_enum)]
        shell: Shell,
    },
}

#[instrument]
#[tokio::main]
async fn main() -> Result<()> {
    registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .from_env_lossy(),
        )
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Get => {
            let oauth_config = OAuthConfig::load().context("Failed to load OAuth configuration")?;
            let mut hosts_config = Hosts::load().context("Failed to load hosts configuration")?;
            commands::get::handle_get(oauth_config, &mut hosts_config, cli.device)
                .await
                .context("Failed to handle 'get' command")?;
        },
        Command::Store => {
            let oauth_config = OAuthConfig::load().context("Failed to load OAuth configuration")?;
            commands::store::handle_store(oauth_config)
                .await
                .context("Failed to handle 'store' command")?;
        },
        Command::Erase => {
            let oauth_config = OAuthConfig::load().context("Failed to load OAuth configuration")?;
            commands::erase::handle_erase(oauth_config)
                .await
                .context("Failed to handle 'erase' command")?;
        },
        Command::List { short } => {
            let profile_config =
                ProfileConfig::load().context("Failed to load profile configuration")?;
            commands::list::list(short, &profile_config).context("Failed to list profiles")?;
        },
        Command::Show { profile: name } => {
            let profile_config =
                ProfileConfig::load().context("Failed to load profile configuration")?;
            commands::show::show(&ProfileRef { name }, &profile_config)
                .context("Failed to show profiles")?;
        },
        Command::Apply { profile: name } => {
            let profile_config =
                ProfileConfig::load().context("Failed to load profile configuration")?;
            commands::apply::apply(name, &profile_config).context("Failed to apply profile")?;
        },
        Command::Login => {
            let oauth_config = OAuthConfig::load().context("Failed to load OAuth configuration")?;
            let mut hosts_config = Hosts::load().context("Failed to load hosts configuration")?;
            commands::login::login(&oauth_config, &mut hosts_config, cli.device)
                .await
                .context("Failed to login")?;
        },
        Command::Logout { hostname, name } => {
            let mut hosts_config = Hosts::load().context("Failed to load hosts configuration")?;
            commands::logout::logout(&mut hosts_config, hostname.as_ref(), name.as_ref())
                .context("Failed to logout")?;
        },
        Command::Refresh { hostname, name } => {
            let oauth_config = OAuthConfig::load().context("Failed to load OAuth configuration")?;
            let hosts_config = Hosts::load().context("Failed to load hosts configuration")?;
            commands::refresh::refresh(
                &oauth_config,
                &hosts_config,
                hostname.as_deref(),
                name.as_deref(),
                cli.device,
            )
            .await
            .context("Failed to refresh credentials")?;
        },
        Command::Switch { hostname, name } => {
            let mut hosts_config = Hosts::load().context("Failed to load hosts configuration")?;
            commands::switch::switch(&mut hosts_config, hostname.as_ref(), name.as_ref())
                .context("Failed to switch credentials")?;
        },
        Command::Status => {
            let hosts_config = Hosts::load().context("Failed to load hosts configuration")?;
            commands::status::status(&hosts_config).context("Failed to show status")?;
        },
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            generate(
                shell,
                &mut cmd,
                env!("CARGO_PKG_NAME"),
                &mut std::io::stderr(),
            );
        },
    }
    Ok(())
}
