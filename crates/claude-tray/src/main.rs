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
//! No tray yet — that arrives in roadmap phase 6. What runs today fetches one
//! snapshot from the chosen provider and prints what the tooltip would say,
//! which is enough to drive the mock through every state the icon will have to
//! render.

use std::process::ExitCode;

use chrono::Local;
use clap::Parser;
use claude_tray_core::{BatteryLevel, ColorState, Thresholds, UsageSnapshot, humanize_duration};
use claude_tray_provider::UsageProvider;

#[derive(Debug, Parser)]
#[command(
    name = "claude-tray",
    version,
    about = "Monitor your Claude usage from the system tray."
)]
struct Cli {
    /// Usage data source to read from.
    ///
    /// Only `mock` exists so far — which real source backs the default is still
    /// open, and waits on the research in roadmap phase 3.
    #[arg(long, value_name = "ID")]
    provider: Option<String>,

    /// Report this percentage of usage remaining instead of reading a real one.
    ///
    /// For walking the icon through every state it has to render. Implies
    /// `--provider mock`.
    #[arg(long, value_name = "PERCENT")]
    simulate_percent: Option<f64>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // A current-thread runtime: there is one fetch in flight and, once phase 6
    // lands, a tray event loop that owns the main thread. Worker threads would
    // have nothing to do.
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return fail(&format!("could not start the async runtime: {error}")),
    };

    match runtime.block_on(run(&cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => fail(&message),
    }
}

/// Reports a failure the way a command-line tool should: on stderr, with a
/// non-zero status.
fn fail(message: &str) -> ExitCode {
    eprintln!("claude-tray: {message}");
    ExitCode::FAILURE
}

/// Fetches one snapshot and prints it.
async fn run(cli: &Cli) -> Result<(), String> {
    // `--simulate-percent` is only meaningful against the mock, so asking for a
    // figure is taken as asking for the provider that can invent one.
    let requested = cli
        .provider
        .as_deref()
        .or_else(|| cli.simulate_percent.map(|_| "mock"));

    let Some(id) = requested else {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        println!("The tray arrives in roadmap phase 6.");
        println!("Meanwhile, try: claude-tray --provider mock --simulate-percent 42");
        return Ok(());
    };

    let provider = build_provider(id, cli.simulate_percent)?;
    let snapshot = provider
        .fetch()
        .await
        .map_err(|error| format!("{id}: {error}"))?;

    print_snapshot(&snapshot);
    Ok(())
}

/// Resolves a provider id to a provider.
// Without the `mock` feature there is no provider left that takes a simulated
// figure, so the argument goes unread rather than unsupported — the flag still
// parses, and the arm below explains why it did nothing.
#[cfg_attr(not(feature = "mock"), allow(unused_variables))]
fn build_provider(
    id: &str,
    simulate_percent: Option<f64>,
) -> Result<Box<dyn UsageProvider>, String> {
    match id {
        #[cfg(feature = "mock")]
        "mock" => mock_provider(simulate_percent),
        #[cfg(not(feature = "mock"))]
        "mock" => Err("this build was compiled without the `mock` feature".to_owned()),
        other => Err(format!(
            "unknown provider `{other}` (available: mock; real providers arrive in roadmap phase 3)"
        )),
    }
}

/// Builds the mock, reporting either the requested figure or the demo one.
#[cfg(feature = "mock")]
fn mock_provider(simulate_percent: Option<f64>) -> Result<Box<dyn UsageProvider>, String> {
    use claude_tray_core::Percent;

    /// The figure reported when none is asked for: the one from the README's
    /// tooltip example.
    const DEMO_PERCENT: f64 = 81.0;

    let remaining = match simulate_percent {
        Some(value) => {
            Percent::new(value).map_err(|error| format!("--simulate-percent: {error}"))?
        }
        None => Percent::new(DEMO_PERCENT).unwrap_or(Percent::FULL),
    };

    Ok(Box::new(claude_tray_provider::MockProvider::fixed(
        remaining,
    )))
}

/// Prints what the tooltip will say once there is a tray to hang it on.
fn print_snapshot(snapshot: &UsageSnapshot) {
    let remaining = snapshot.remaining_percent();
    let color = ColorState::from_remaining(remaining, Thresholds::default());

    let gauge = match remaining {
        Some(percent) => format!(
            "{percent}%  {}  ({color})",
            BatteryLevel::from_percent(percent)
        ),
        None => format!("—  ({color})"),
    };

    println!("Claude {}", snapshot.plan);
    println!();
    print_row("Remaining", &gauge);
    print_row(
        "Resets in",
        &humanize_duration(snapshot.time_until_reset(snapshot.captured_at)),
    );
    print_row("Model", snapshot.model.as_deref().unwrap_or("—"));
    print_row("Source", snapshot.source.as_str());
    print_row(
        "Last updated",
        // The one place local time is right: a timestamp being read by a person
        // sitting in front of this machine.
        &snapshot
            .captured_at
            .with_timezone(&Local)
            .format("%H:%M:%S")
            .to_string(),
    );
}

/// Prints one aligned label/value row.
fn print_row(label: &str, value: &str) {
    println!("{label:<14}{value}");
}
