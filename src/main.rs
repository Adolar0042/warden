#![cfg_attr(doc, doc = include_str!("../README.md"))]

use anyhow::Result;
use clap::Parser as _;
use tracing::instrument;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, fmt, registry};

use crate::cli::Cli;

mod cli;
mod commands;
mod config;
mod keyring;
mod oauth;
mod profile;
mod theme;
mod utils;

#[instrument]
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(cli.verbosity.tracing_level_filter().into())
                .from_env_lossy(),
        )
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    cli.command.run(cli.device).await?;
    Ok(())
}
