//! Central configuration module.
//!
//! This splits the previously monolithic `config.rs` into a structured module.
//!
//! Layout:
//! - `provider`: OAuth provider configuration loading & validation
//! - `git_source`: `config::Source` implementation for Git-based provider
//!   overrides
//! - `hosts`: host/credential state
//! - `profiles`: profile & rule configuration
//!
//! The public types re‑exported here mirror the old API so other modules
//! require minimal (ideally zero) changes.
//!
//! NOTE: This file only wires submodules together; logic lives in the
//! respective files.
//!
//! Follow‑up: migrate callers to use submodule paths directly if desired.

pub mod git_source;
pub mod hosts;
pub mod profiles;
pub mod provider;

pub use hosts::Hosts;
pub use profiles::ProfileConfig;
pub use provider::{OAuthConfig, ProviderConfig};
