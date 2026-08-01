//! Times repeated fetches from the local-file provider against this machine's
//! real Claude Code directory.
//!
//! The first pass pays for all the history inside the longest window; every pass
//! after it should read only what has been appended since, which is what keeps a
//! 30-second refresh inside the idle-CPU budget in `docs/PERFORMANCE.md`.
//!
//! ```sh
//! cargo run --release --example scan-timing -p claude-tray-provider
//! ```

use std::time::Instant;

use claude_tray_provider::{LocalFileProvider, UsageProvider};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = LocalFileProvider::discover()?;
    println!("reading {}", provider.root().display());

    for pass in 1..=5 {
        let started = Instant::now();
        let snapshot = provider.fetch().await?;
        let elapsed = started.elapsed();
        let report = provider.last_scan().unwrap_or_default();

        println!(
            "pass {pass}: {elapsed:>10.2?}  {seen} files ({read} read, {skipped} skipped)  \
             {bytes:>10} bytes  +{added} records ({retained} retained)  windows: {windows:?}",
            seen = report.files_seen,
            read = report.files_read,
            skipped = report.files_skipped,
            bytes = report.bytes_read,
            added = report.records_added,
            retained = report.records_retained,
            windows = snapshot
                .windows
                .iter()
                .map(|window| (window.kind, window.used))
                .collect::<Vec<_>>(),
        );
    }

    Ok(())
}
