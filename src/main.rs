#![cfg_attr(doc, doc = include_str!("../README.md"))]

use anyhow::Result;
use clap::Parser as _;
use tracing::instrument;
use tracing::level_filters::LevelFilter;
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
    registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .from_env_lossy(),
        )
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    let Cli { command, device } = Cli::parse();
    command.run(device).await?;
    Ok(())
}
