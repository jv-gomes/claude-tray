//! Claude Tray — monitor your Claude usage from the system tray.
//!
//! This binary is the composition root. It owns the tray icon and the platform
//! event loop, and wires together the domain model, the usage providers, and the
//! icon renderer.
//!
//! # Threading
//!
//! Two execution contexts, joined by a channel:
//!
//! - The **UI thread** owns the tray handle and the event loop. On macOS and
//!   Windows the tray object must live on the main thread. This thread only paints
//!   and handles menu clicks; it never performs I/O and never blocks.
//! - A **background runtime** owns the refresh scheduler, the providers, and all
//!   file and network access, and reports results back as events.
//!
//! # Status
//!
//! Skeleton only. The tray and event loop arrive in roadmap phase 6.

fn main() {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    println!("Skeleton build — the tray arrives in roadmap phase 6.");
}
