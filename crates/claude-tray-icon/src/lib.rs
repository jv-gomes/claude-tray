//! Battery-style tray icon rendering for Claude Tray.
//!
//! Turns a usage state into the RGBA pixels the platform tray expects: a battery
//! shell whose fill drops in ten discrete steps and whose colour moves from green
//! through yellow and orange to red as the remaining quota falls.
//!
//! # Design notes
//!
//! - **Fill level carries the signal, colour reinforces it.** Colour alone must
//!   never be load-bearing, so the icon stays readable for colour-blind users and
//!   in monochrome themes.
//! - **Render at the device pixel ratio.** A small raster scaled up looks wrong on
//!   HiDPI panels, so icons are rendered at the size actually needed.
//! - **Results are cached** by icon specification, because the tray re-renders far
//!   more often than the underlying state changes.
//! - **macOS needs a template image** — monochrome plus alpha — so the menu bar can
//!   tint it correctly in light and dark appearances.
//!
//! # Status
//!
//! Skeleton only. Rendering arrives in roadmap phase 5.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
