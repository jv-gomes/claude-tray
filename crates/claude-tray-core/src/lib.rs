//! Core domain model for Claude Tray.
//!
//! This crate holds everything that can be decided without touching the outside
//! world: how a usage snapshot becomes a percentage, which colour and battery
//! level that percentage maps to, when a notification should fire, and what the
//! tooltip should say.
//!
//! # The one rule
//!
//! **This crate has no I/O dependencies.** No network, no filesystem, no display,
//! and no ambient clock — anything time-dependent takes the current instant as a
//! parameter. That constraint is what allows the majority of the project's logic
//! to be unit-tested in CI on every platform without a desktop session, a network
//! connection, or Claude credentials.
//!
//! Anything that needs to talk to the world belongs in `claude-tray-provider`
//! (data sources) or the `claude-tray` binary (tray, config, logging).
//!
//! # Status
//!
//! Skeleton only. The types described above arrive in roadmap phase 2.

// Tests are allowed the ergonomic panicking helpers that library code is not.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
