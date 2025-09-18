use anyhow::{Context as _, Result};
use clap::{CommandFactory as _, Parser, Subcommand};
use clap_complete::{Shell, generate};

use crate::commands;
use crate::profile::rule::ProfileRef;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Use OAuth device flow or fail
    #[clap(short, long, global = true)]
    pub device: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
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
        /// Do not attempt to infer and filter by the host from the remote URL
        #[clap(short, long)]
        all: bool,
    },
    /// Show the current status of the credentials.
    Status,
    /// Generate shell completions for the given shell.
    Completions {
        #[clap(value_enum)]
        shell: Shell,
    },
}

impl Command {
    pub async fn run(self, force_device: bool) -> Result<()> {
        match self {
            Self::Get => {
                commands::get::handle_get(force_device)
                    .await
                    .context("Failed to handle 'get' command")?;
            },
            Self::Store => {
                commands::store::handle_store()
                    .await
                    .context("Failed to handle 'store' command")?;
            },
            Self::Erase => {
                commands::erase::handle_erase()
                    .await
                    .context("Failed to handle 'erase' command")?;
            },
            Self::List { short } => {
                commands::list::list(short).context("Failed to list profiles")?;
            },
            Self::Show { profile: name } => {
                commands::show::show(&ProfileRef { name }).context("Failed to show profiles")?;
            },
            Self::Apply { profile: name } => {
                commands::apply::apply(name).context("Failed to apply profile")?;
            },
            Self::Login => {
                commands::login::login(force_device)
                    .await
                    .context("Failed to perform login")?;
            },
            Self::Logout { hostname, name } => {
                commands::logout::logout(hostname.as_ref(), name.as_ref())
                    .context("Failed to perform logout")?;
            },
            Self::Refresh { hostname, name } => {
                commands::refresh::refresh(hostname.as_deref(), name.as_deref(), force_device)
                    .await
                    .context("Failed to refresh credential")?;
            },
            Self::Switch {
                hostname,
                name,
                all,
            } => {
                commands::switch::switch(hostname.as_ref(), name.as_ref(), all)
                    .context("Failed to switch credential")?;
            },
            Self::Status => {
                commands::status::status().context("Failed to show credential status")?;
            },
            Self::Completions { shell } => {
                let mut cmd = Cli::command();
                generate(
                    shell,
                    &mut cmd,
                    env!("CARGO_PKG_NAME"),
                    &mut std::io::stdout(),
                );
            },
        }
        Ok(())
    }
}
