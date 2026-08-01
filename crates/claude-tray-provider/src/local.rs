//! Reading usage out of Claude Code's own files on this machine.

use core::time::Duration;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use claude_tray_core::{Source, UsageSnapshot, UsageWindow, WindowKind};

use crate::{Health, ProviderCapabilities, ProviderError, UsageProvider};

/// The environment variable Claude Code honours for a non-default config
/// directory.
pub const CONFIG_DIR_ENV: &str = "CLAUDE_CONFIG_DIR";

/// The directory Claude Code keeps its state in, relative to the home
/// directory.
const DEFAULT_DIR_NAME: &str = ".claude";

/// Where transcripts live inside the config directory.
const PROJECTS_DIR: &str = "projects";

/// Reads usage from Claude Code's transcripts under `~/.claude/projects`.
///
/// # What this can and cannot tell you
///
/// **It reports consumption, not a percentage.** The transcripts record how
/// many tokens each turn cost, and nothing else: there is no quota, no
/// allowance, and no reset time anywhere in the directory. So every window this
/// provider produces has [`UsageWindow::limit`] set to `None`, which the domain
/// model correctly renders as an unknown state rather than as a full battery.
///
/// That is a property of the data, not a gap in this parser. Anything that draws
/// a percentage needs a source that knows the allowance — see
/// `docs/DATA-SOURCES.md`.
///
/// It also says nothing about the subscription: [`UsageSnapshot::plan`] is
/// always `None`. The plan *is* recorded on this machine, but only inside
/// `.credentials.json` alongside the OAuth tokens, and reading a secret file to
/// learn a display string is not a trade this provider makes. Phase 4 owns that
/// file.
///
/// # What it does report
///
/// Tokens consumed within each of several rolling spans — the last five hours,
/// seven days, and thirty days by default — which is enough to answer "how hard
/// have I been leaning on this today" even without a limit to compare against.
#[derive(Debug)]
pub struct LocalFileProvider {
    id: String,
    root: PathBuf,
    spans: Vec<Span>,
    now: Option<DateTime<Utc>>,
    filter_by_mtime: bool,
    last_scan: Mutex<Option<ScanReport>>,
}

/// One rolling window this provider aggregates over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    kind: WindowKind,
    length: Duration,
}

/// What a pass over the transcripts found.
///
/// Kept so that [`LocalFileProvider::health`] can report a parser that has
/// stopped recognising the files, which is the failure mode to expect from an
/// undocumented on-disk format. Phase 12 will route this to the log as well.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanReport {
    /// Transcript files opened.
    pub files_scanned: usize,
    /// Files skipped because they had not been written to recently enough to
    /// hold a record inside any window.
    pub files_skipped: usize,
    /// Records that carried a usable usage block.
    pub records_with_usage: usize,
    /// Lines that were not valid JSON. A few are expected — a transcript being
    /// appended to while it is read can end mid-line.
    pub unparseable_lines: usize,
}

impl ScanReport {
    /// Whether the parser found nothing at all where it expected records.
    ///
    /// True after reading files that contained no recognisable usage, which is
    /// what a change to the on-disk format would look like from here.
    #[must_use]
    pub fn looks_unrecognised(&self) -> bool {
        self.files_scanned > 0 && self.records_with_usage == 0
    }
}

impl LocalFileProvider {
    /// The provider's id, as configuration refers to it.
    pub const ID: &'static str = "local-file";

    /// The shape of the transcripts this parser was written against.
    ///
    /// Bump it when the parsing changes, so that a bug report can say which
    /// reading of the files produced a figure. The format is Claude Code's
    /// internal one and carries no compatibility promise.
    pub const PARSER_VERSION: u32 = 1;

    /// The rolling spans reported unless told otherwise.
    const DEFAULT_SPANS: [Span; 3] = [
        Span {
            kind: WindowKind::Session,
            length: Duration::from_hours(5),
        },
        Span {
            kind: WindowKind::Weekly,
            length: Duration::from_hours(7 * 24),
        },
        Span {
            kind: WindowKind::Monthly,
            length: Duration::from_hours(30 * 24),
        },
    ];

    /// Locates the Claude Code directory the way Claude Code itself does.
    ///
    /// `CLAUDE_CONFIG_DIR` wins if it is set; otherwise `~/.claude`. Note that
    /// this is *not* the platform's config location — Claude Code uses the home
    /// directory on Windows and macOS too, so following the XDG or Known Folders
    /// convention here would look in a directory that is never populated.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::internal`] if there is no home directory to
    /// resolve and no override was given.
    pub fn discover() -> Result<Self, ProviderError> {
        Self::resolve_root(std::env::var_os(CONFIG_DIR_ENV)).map(Self::at)
    }

    /// The directory-resolution rule, with the environment passed in.
    ///
    /// Separated from [`Self::discover`] so it can be tested without setting a
    /// process-global variable — which in a threaded test binary is both racy
    /// and, since the 2024 edition, `unsafe`.
    fn resolve_root(configured: Option<std::ffi::OsString>) -> Result<PathBuf, ProviderError> {
        match configured {
            Some(value) if !value.is_empty() => Ok(PathBuf::from(value)),
            _ => Ok(directories::BaseDirs::new()
                .ok_or_else(|| {
                    ProviderError::internal(format!(
                        "no home directory to look for `{DEFAULT_DIR_NAME}` in, and \
                         {CONFIG_DIR_ENV} is not set"
                    ))
                })?
                .home_dir()
                .join(DEFAULT_DIR_NAME)),
        }
    }

    /// Reads from a given directory, bypassing discovery.
    #[must_use]
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self {
            id: Self::ID.to_owned(),
            root: root.into(),
            spans: Self::DEFAULT_SPANS.to_vec(),
            now: None,
            filter_by_mtime: true,
            last_scan: Mutex::new(None),
        }
    }

    /// Renames the provider.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Replaces the rolling spans that get aggregated.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::internal`] if no spans are given, since a
    /// provider that aggregates nothing can only report an empty snapshot.
    pub fn with_spans(
        mut self,
        spans: impl IntoIterator<Item = (WindowKind, Duration)>,
    ) -> Result<Self, ProviderError> {
        let spans: Vec<Span> = spans
            .into_iter()
            .map(|(kind, length)| Span { kind, length })
            .collect();

        if spans.is_empty() {
            return Err(ProviderError::internal(
                "a local-file provider needs at least one window to aggregate over",
            ));
        }

        self.spans = spans;
        Ok(self)
    }

    /// Pins the instant the windows are measured back from.
    ///
    /// Also switches off the modification-time filter: that filter compares file
    /// metadata against the wall clock, which a pinned instant does not agree
    /// with, and a test that silently skipped its own fixtures would pass for
    /// the wrong reason.
    #[must_use]
    pub fn with_now(mut self, now: DateTime<Utc>) -> Self {
        self.now = Some(now);
        self.filter_by_mtime = false;
        self
    }

    /// Returns the directory being read.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns what the last fetch found, if there has been one.
    #[must_use]
    pub fn last_scan(&self) -> Option<ScanReport> {
        match self.last_scan.lock() {
            Ok(guard) => *guard,
            // A poisoned lock means some other thread panicked mid-report. The
            // report is only diagnostic, so the stale value is better than
            // propagating someone else's panic.
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    /// The longest span, which bounds how far back a file can be useful.
    fn longest_span(&self) -> Duration {
        self.spans
            .iter()
            .map(|span| span.length)
            .max()
            .unwrap_or(Duration::ZERO)
    }

    /// Walks the transcripts and totals tokens per span.
    fn scan(&self, projects: &Path, now: DateTime<Utc>) -> (Vec<u64>, Option<String>, ScanReport) {
        let mut totals = vec![0_u64; self.spans.len()];
        let mut report = ScanReport::default();
        let mut newest: Option<(DateTime<Utc>, String)> = None;

        let cutoff = now
            .checked_sub_signed(
                chrono::TimeDelta::from_std(self.longest_span()).unwrap_or(chrono::TimeDelta::MAX),
            )
            .unwrap_or(now);

        for path in transcripts(projects) {
            if self.filter_by_mtime && !modified_since(&path, cutoff) {
                report.files_skipped += 1;
                continue;
            }

            let Ok(file) = fs::File::open(&path) else {
                // A transcript that cannot be opened — deleted mid-scan, or
                // belonging to another user — is not a reason to report no
                // usage at all.
                continue;
            };
            report.files_scanned += 1;

            for line in BufReader::new(file).lines() {
                let Ok(line) = line else {
                    report.unparseable_lines += 1;
                    continue;
                };

                // Most of a transcript is prompts, responses, and file
                // snapshots. Deserialising all of it to find the token counters
                // would be reading tens of megabytes to use a few kilobytes.
                if !line.contains("\"usage\"") {
                    continue;
                }

                let Ok(record) = serde_json::from_str::<serde_json::Value>(&line) else {
                    report.unparseable_lines += 1;
                    continue;
                };

                let Some((timestamp, tokens, model)) = read_record(&record) else {
                    continue;
                };
                report.records_with_usage += 1;

                if newest.as_ref().is_none_or(|(seen, _)| timestamp > *seen)
                    && let Some(model) = model
                {
                    newest = Some((timestamp, model));
                }

                for (index, span) in self.spans.iter().enumerate() {
                    if within(timestamp, now, span.length)
                        && let Some(total) = totals.get_mut(index)
                    {
                        *total += tokens;
                    }
                }
            }
        }

        (totals, newest.map(|(_, model)| model), report)
    }

    /// Records what a scan found, for [`Self::last_scan`] and [`Self::health`].
    fn remember(&self, report: ScanReport) {
        match self.last_scan.lock() {
            Ok(mut guard) => *guard = Some(report),
            Err(poisoned) => *poisoned.into_inner() = Some(report),
        }
    }
}

#[async_trait]
impl UsageProvider for LocalFileProvider {
    fn id(&self) -> &str {
        &self.id
    }

    /// Windows but no limits, and therefore no percentage.
    ///
    /// This is the honest declaration that lets the tooltip show a token count
    /// and omit the "remaining" row, rather than showing a row that would never
    /// be filled.
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            windows: self.spans.iter().map(|span| span.kind).collect(),
            limits: false,
            reset_times: false,
            plan: false,
            model: true,
        }
    }

    async fn fetch(&self) -> Result<UsageSnapshot, ProviderError> {
        if !self.root.is_dir() {
            return Err(ProviderError::not_authenticated(format!(
                "{} does not exist — Claude Code has not run on this machine",
                self.root.display()
            )));
        }

        let projects = self.root.join(PROJECTS_DIR);
        if !projects.is_dir() {
            return Err(ProviderError::not_authenticated(format!(
                "{} has no `{PROJECTS_DIR}` directory — nothing has been recorded yet",
                self.root.display()
            )));
        }

        let now = self.now.unwrap_or_else(Utc::now);
        let (totals, model, report) = self.scan(&projects, now);
        self.remember(report);

        if report.looks_unrecognised() {
            return Err(ProviderError::parse_failed(format!(
                "read {} transcript(s) under {} without finding a single usage record; \
                 the on-disk format may have changed (parser version {})",
                report.files_scanned,
                projects.display(),
                Self::PARSER_VERSION,
            )));
        }

        let windows = self
            .spans
            .iter()
            .zip(totals)
            // No limit and no reset time, because the files record neither.
            .map(|(span, used)| UsageWindow::unbounded(span.kind, used))
            .collect();

        let snapshot = UsageSnapshot::new(now, windows, Source::new(self.id.clone()));
        Ok(match model {
            Some(model) => snapshot.with_model(model),
            None => snapshot,
        })
    }

    async fn health(&self) -> Health {
        if !self.root.join(PROJECTS_DIR).is_dir() {
            return Health::Unavailable {
                reason: format!("no transcripts under {}", self.root.display()),
            };
        }

        match self.last_scan() {
            Some(report) if report.looks_unrecognised() => Health::Degraded {
                reason: format!(
                    "transcripts are no longer in a recognised shape (parser version {})",
                    Self::PARSER_VERSION
                ),
            },
            _ => Health::Ready,
        }
    }
}

/// Every transcript under the projects directory, in whatever order the
/// filesystem gives them.
fn transcripts(projects: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![projects.to_path_buf()];

    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => pending.push(path),
                Ok(_) if path.extension().is_some_and(|ext| ext == "jsonl") => found.push(path),
                _ => {}
            }
        }
    }

    found
}

/// Whether a file has been written to recently enough to hold a record inside
/// the longest window.
///
/// Transcripts are append-only, so one last touched before the cutoff cannot
/// contain a record after it. On a machine with months of history this is what
/// keeps a refresh from re-reading every megabyte ever written.
fn modified_since(path: &Path, cutoff: DateTime<Utc>) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_or(true, |modified| DateTime::<Utc>::from(modified) >= cutoff)
}

/// Whether an instant falls inside a window of the given length ending now.
///
/// A record stamped in the future — clock changes, a file copied from another
/// machine — is left out rather than counted against the current window.
fn within(timestamp: DateTime<Utc>, now: DateTime<Utc>, length: Duration) -> bool {
    let age = now.signed_duration_since(timestamp);
    age >= chrono::TimeDelta::zero() && age.to_std().is_ok_and(|elapsed| elapsed <= length)
}

/// Pulls the timestamp, token total, and model out of one transcript record.
///
/// Every field is treated as optional and every unexpected shape returns `None`
/// rather than failing: this is an undocumented format, and a record we do not
/// understand should cost one record, not the whole reading.
fn read_record(record: &serde_json::Value) -> Option<(DateTime<Utc>, u64, Option<String>)> {
    let message = record.get("message")?;
    let usage = message.get("usage")?;

    let timestamp = record
        .get("timestamp")?
        .as_str()?
        .parse::<DateTime<Utc>>()
        .ok()?;

    let tokens = COUNTED_TOKEN_FIELDS
        .iter()
        .filter_map(|field| usage.get(*field))
        .filter_map(serde_json::Value::as_u64)
        .sum();

    let model = message
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);

    Some((timestamp, tokens, model))
}

/// The token counters that get added up.
///
/// `cache_read_input_tokens` is deliberately excluded. It counts context re-read
/// from cache, it is priced at a fraction of the rest, and on a real transcript
/// it runs two orders of magnitude larger than everything else combined — so
/// including it would produce a number that moves with how much context was
/// reused rather than with how much work was done.
const COUNTED_TOKEN_FIELDS: [&str; 3] = [
    "input_tokens",
    "output_tokens",
    "cache_creation_input_tokens",
];

#[cfg(test)]
mod tests {
    use super::{LocalFileProvider, ScanReport};
    use crate::{Health, UsageProvider};
    use chrono::{DateTime, Utc};
    use claude_tray_core::WindowKind;
    use core::time::Duration;
    use std::path::PathBuf;

    /// The instant the fixture transcripts are written relative to.
    const NOW: &str = "2026-08-01T12:00:00Z";

    fn instant(text: &str) -> DateTime<Utc> {
        text.parse().unwrap()
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn provider(name: &str) -> LocalFileProvider {
        LocalFileProvider::at(fixture(name)).with_now(instant(NOW))
    }

    #[tokio::test]
    async fn it_totals_tokens_per_rolling_window() {
        let snapshot = provider("claude-home").fetch().await.unwrap();

        // Fixture: 300 tokens 1h ago, 600 tokens 2 days ago, 1500 tokens 10
        // days ago, and 9000 tokens 100 days ago which is outside every window.
        assert_eq!(
            snapshot.window(WindowKind::Session).map(|w| w.used),
            Some(300)
        );
        assert_eq!(
            snapshot.window(WindowKind::Weekly).map(|w| w.used),
            Some(900)
        );
        assert_eq!(
            snapshot.window(WindowKind::Monthly).map(|w| w.used),
            Some(2400)
        );
    }

    /// The whole point of the research spike: these files record consumption,
    /// not a quota. A window with an invented limit would put a confident
    /// percentage on the icon with nothing behind it.
    #[tokio::test]
    async fn no_window_claims_a_limit_or_a_reset_time() {
        let snapshot = provider("claude-home").fetch().await.unwrap();

        for window in &snapshot.windows {
            assert_eq!(window.limit, None, "{:?}", window.kind);
            assert_eq!(window.resets_at, None, "{:?}", window.kind);
            assert!(!window.is_measurable());
        }

        assert_eq!(
            snapshot.remaining_percent(),
            None,
            "there is no percentage to be had from this source"
        );
    }

    #[tokio::test]
    async fn it_reports_the_model_from_the_most_recent_record() {
        let snapshot = provider("claude-home").fetch().await.unwrap();
        assert_eq!(snapshot.model.as_deref(), Some("claude-opus-5"));
    }

    /// The plan is on this machine, but only inside the credential file. Naming
    /// a plan we did not read would be a guess.
    #[tokio::test]
    async fn it_does_not_claim_to_know_the_plan() {
        let snapshot = provider("claude-home").fetch().await.unwrap();

        assert_eq!(snapshot.plan, None);
        assert!(!provider("claude-home").capabilities().plan);
    }

    #[tokio::test]
    async fn capabilities_admit_there_is_no_percentage() {
        let capabilities = provider("claude-home").capabilities();

        assert!(!capabilities.limits);
        assert!(!capabilities.reports_percentage());
        assert!(capabilities.supports_window(WindowKind::Session));
        assert!(capabilities.model);
    }

    /// Cache reads run two orders of magnitude above every other counter, so
    /// including them would produce a figure that tracks context reuse rather
    /// than work done.
    #[tokio::test]
    async fn cache_reads_are_not_counted() {
        let snapshot = provider("claude-home").fetch().await.unwrap();
        let session = snapshot.window(WindowKind::Session).unwrap();

        assert_eq!(
            session.used, 300,
            "the fixture's recent record carries 1,000,000 cache-read tokens alongside its 300"
        );
    }

    /// An undocumented format will produce records we do not understand. One
    /// should cost one record, not the whole reading.
    #[tokio::test]
    async fn damaged_records_are_skipped_rather_than_fatal() {
        let provider = provider("damaged-home");
        let snapshot = provider.fetch().await.unwrap();
        let report = provider.last_scan().unwrap();

        assert_eq!(
            snapshot.window(WindowKind::Session).map(|w| w.used),
            Some(150),
            "the one intact record still counts"
        );
        assert!(report.unparseable_lines >= 1);
        assert_eq!(report.records_with_usage, 1);
    }

    /// What a change to the on-disk format looks like from here: files present,
    /// nothing recognised in them.
    #[tokio::test]
    async fn transcripts_with_no_usage_at_all_are_reported_as_a_parse_failure() {
        let provider = provider("unrecognised-home");
        let error = provider.fetch().await.unwrap_err();

        assert_eq!(
            error.kind(),
            claude_tray_core::ErrorKind::ParseFailed,
            "{error}"
        );
        assert!(
            error.message().contains("parser version"),
            "a bug report needs to say which reading produced this: {}",
            error.message()
        );
    }

    #[tokio::test]
    async fn a_missing_directory_reads_as_not_signed_in() {
        let provider = LocalFileProvider::at(fixture("does-not-exist"));
        let error = provider.fetch().await.unwrap_err();

        assert_eq!(error.kind(), claude_tray_core::ErrorKind::NotAuthenticated);
        assert!(!error.is_retryable(), "waiting will not create the files");
        assert!(!provider.health().await.is_usable());
    }

    #[tokio::test]
    async fn a_directory_without_transcripts_reads_as_not_signed_in() {
        let error = provider("empty-home").fetch().await.unwrap_err();
        assert_eq!(error.kind(), claude_tray_core::ErrorKind::NotAuthenticated);
    }

    #[tokio::test]
    async fn health_turns_degraded_once_a_scan_finds_nothing_recognisable() {
        let provider = provider("unrecognised-home");

        assert_eq!(provider.health().await, Health::Ready, "before any scan");
        provider.fetch().await.unwrap_err();
        assert!(matches!(provider.health().await, Health::Degraded { .. }));
    }

    #[tokio::test]
    async fn the_windows_can_be_reconfigured() {
        let provider = LocalFileProvider::at(fixture("claude-home"))
            .with_now(instant(NOW))
            .with_spans([(WindowKind::Session, Duration::from_hours(365 * 24))])
            .unwrap();

        let snapshot = provider.fetch().await.unwrap();

        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(
            snapshot.window(WindowKind::Session).map(|w| w.used),
            Some(11_400),
            "a year-long window takes in the record that every default window excludes"
        );
    }

    #[tokio::test]
    async fn a_provider_with_no_windows_is_rejected() {
        assert!(
            LocalFileProvider::at(fixture("claude-home"))
                .with_spans([])
                .is_err()
        );
    }

    #[test]
    fn an_unscanned_provider_has_nothing_to_report() {
        assert_eq!(
            LocalFileProvider::at(fixture("claude-home")).last_scan(),
            None
        );
        assert!(!ScanReport::default().looks_unrecognised());
    }

    #[test]
    fn an_override_wins_over_the_home_directory() {
        let root = LocalFileProvider::resolve_root(Some("/somewhere/else".into())).unwrap();
        assert_eq!(root, PathBuf::from("/somewhere/else"));
    }

    /// An unset variable and one set to nothing are the same instruction.
    #[test]
    fn an_empty_override_falls_back_to_the_home_directory() {
        let empty = LocalFileProvider::resolve_root(Some(String::new().into())).unwrap();
        let unset = LocalFileProvider::resolve_root(None).unwrap();

        assert_eq!(empty, unset);
        assert!(
            unset.ends_with(".claude"),
            "Claude Code uses the home directory on every platform, not the XDG or \
             Known Folders config location: {}",
            unset.display()
        );
    }

    #[test]
    fn a_discovered_provider_uses_the_configured_id() {
        assert_eq!(LocalFileProvider::discover().unwrap().id(), "local-file");
    }
}
