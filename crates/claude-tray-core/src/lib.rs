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
//! # The model
//!
//! A provider produces a [`UsageSnapshot`]: the [`Plan`] it belongs to, the
//! [`UsageWindow`]s it reported, and the instant it was captured. The
//! application holds exactly one [`UsageState`], which is either a snapshot
//! (current or stale) or a reason there isn't one.
//!
//! # From a snapshot to an icon
//!
//! [`UsageSnapshot::remaining_percent`] picks the most constrained window and
//! reports what is left of it. That single [`Percent`] becomes a
//! [`ColorState`] via configurable [`Thresholds`] and a [`BatteryLevel`] via
//! quantisation, each optionally passed through [`Hysteresis`] so a figure
//! resting on a boundary does not flicker the icon.
//!
//! # Status
//!
//! The notification engine arrives in the rest of roadmap phase 2.

// Tests are allowed the ergonomic panicking helpers that library code is not.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod battery;
mod color;
mod duration;
mod hysteresis;
mod percent;
mod plan;
mod snapshot;
mod state;
mod window;

pub use battery::BatteryLevel;
pub use color::{ColorState, Thresholds, ThresholdsError};
pub use duration::{UNKNOWN, humanize_duration};
pub use hysteresis::Hysteresis;
pub use percent::{Percent, PercentError};
pub use plan::Plan;
pub use snapshot::{Source, UsageSnapshot};
pub use state::{ErrorKind, UsageState};
pub use window::{UsageWindow, WindowKind};
