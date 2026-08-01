//! Battery-style tray icon rendering for Claude Tray.
//!
//! Turns a usage state into the RGBA pixels the platform tray expects: a battery
//! shell whose fill drops in ten discrete steps and whose colour moves from green
//! through yellow and orange to red as the remaining quota falls.
//!
//! ```
//! use claude_tray_core::{Percent, Thresholds};
//! use claude_tray_icon::{IconRenderer, IconSize, IconSpec, Theme};
//!
//! let renderer = IconRenderer::new();
//! let spec = IconSpec::from_percent(
//!     Percent::new(81.0)?,
//!     Thresholds::default(),
//!     Theme::Dark,
//!     IconSize::for_scale(22, 2.0),
//! );
//!
//! let icon = renderer.render(spec)?;
//! assert_eq!(icon.width(), 44);
//! assert_eq!(icon.rgba().len(), 44 * 44 * 4);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Design notes
//!
//! - **Fill level carries the signal, colour reinforces it.** Colour alone must
//!   never be load-bearing, so the icon stays readable for colour-blind users and
//!   in monochrome themes.
//! - **Render at the device pixel ratio.** A small raster scaled up looks wrong on
//!   HiDPI panels, so icons are rendered at the size actually needed —
//!   [`IconSize::for_scale`] rather than a nearest-size lookup.
//! - **Results are cached** by icon specification, because the tray re-renders far
//!   more often than the underlying state changes.
//! - **macOS needs a template image** — monochrome plus alpha — so the menu bar can
//!   tint it correctly in light and dark appearances. That is [`Theme::Template`],
//!   and phase 6 must mark the image accordingly.
//!
//! Everything is drawn procedurally with `tiny-skia`; there is no SVG and there
//! are no baked assets. See `docs/adr/0005-icon-pipeline.md`.
//!
//! # No platform access
//!
//! This crate does not ask the system anything. Whether the panel is light or
//! dark, and what the accent colour is, arrive as arguments —
//! [`Preference::resolve`] turns them into a drawable [`Theme`]. That keeps every
//! state renderable and testable in CI without a desktop session.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod icon;
mod render;
#[cfg(test)]
mod render_tests;
mod spec;
mod theme;

pub use icon::{Icon, IconRenderer, PngError, RenderError};
pub use spec::{IconSize, IconSizeError, IconSpec, Overlay};
pub use theme::{Appearance, Palette, Preference, Rgb, Theme};
