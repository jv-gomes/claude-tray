//! Usage data sources for Claude Tray.
//!
//! Every source of usage information implements a single provider trait, and the
//! rest of the application depends only on that trait and on the domain types in
//! [`claude_tray_core`].
//!
//! # Why the indirection
//!
//! There is no documented public API exposing usage and quota for a consumer
//! Claude subscription. The candidate sources differ in coverage, in stability,
//! and in standing — and whichever is chosen, it will eventually change. Routing
//! everything through one trait means a source breaking is contained to a single
//! file, and it lets the entire user interface be built against a mock while the
//! question is still open.
//!
//! See `docs/adr/0002-provider-abstraction.md` for the full reasoning.
//!
//! # Provider classes
//!
//! - **Supported** — built on documented or local, stable data. Enabled by default.
//! - **Best-effort** — built on undocumented behaviour. Gated behind a non-default
//!   cargo feature, disabled in configuration, and requiring an explicit informed
//!   opt-in before it will run.
//!
//! # Composition
//!
//! [`ProviderRegistry`] turns the provider ids in configuration into providers,
//! and [`FallbackChain`] tries several in order until one answers. A chain is
//! itself a [`UsageProvider`], so the rest of the application holds exactly one
//! regardless of how many sources are configured.
//!
//! # Status
//!
//! [`UsageProvider`] and [`MockProvider`] exist. Which real source backs v1.0 is
//! still open — it waits on the research spikes in roadmap phase 3.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod error;
mod fallback;
#[cfg(feature = "mock")]
mod mock;
mod provider;
mod registry;

pub use error::ProviderError;
pub use fallback::FallbackChain;
#[cfg(feature = "mock")]
pub use mock::{Fault, MockProvider, Step};
pub use provider::{Health, ProviderCapabilities, UsageProvider};
pub use registry::{ProviderRegistry, RegistryError};
