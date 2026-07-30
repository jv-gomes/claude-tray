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
//! # Status
//!
//! Skeleton only. The provider trait and the mock arrive in roadmap phase 3.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
