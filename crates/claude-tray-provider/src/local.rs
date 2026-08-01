//! Reading usage out of Claude Code's own files on this machine.

use core::time::Duration;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
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
    /// What has already been parsed out of each transcript, so a refresh only
    /// reads what has been appended since.
    cache: Mutex<HashMap<PathBuf, FileState>>,
}

/// One rolling window this provider aggregates over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    kind: WindowKind,
    length: Duration,
}

/// What a pass over the transcripts did.
///
/// Kept for two reasons: so [`LocalFileProvider::health`] can report a parser
/// that has stopped recognising the files, which is the failure mode to expect
/// from an undocumented on-disk format; and so the incremental reading can be
/// checked to be working, since a refresh that quietly re-read everything would
/// still produce the right answer. Phase 12 will route this to the log as well.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanReport {
    /// Transcript files found under the projects directory.
    pub files_seen: usize,
    /// Files that had grown since the last pass and were read from.
    pub files_read: usize,
    /// Files never read, because they were last written to before the longest
    /// window began.
    pub files_skipped: usize,
    /// Bytes actually read, which on a steady-state refresh is only what has
    /// been appended since the previous one.
    pub bytes_read: u64,
    /// Usage records parsed during this pass.
    pub records_added: usize,
    /// Usage records held in memory afterwards, across every window.
    pub records_retained: usize,
    /// Lines that were not valid JSON. A few are expected — a transcript can be
    /// appended to while it is being read.
    pub unparseable_lines: usize,
}

impl ScanReport {
    /// Whether the parser read files and has nothing to show for it.
    ///
    /// Judged on what is *retained* rather than on what this pass added, so that
    /// a refresh which picks up an ordinary non-usage line — a prompt, a file
    /// snapshot — is not mistaken for a format change.
    #[must_use]
    pub fn looks_unrecognised(&self) -> bool {
        self.files_read > 0 && self.records_retained == 0
    }
}

/// What is remembered about one transcript between refreshes.
#[derive(Debug, Default)]
struct FileState {
    /// How many bytes have been parsed. Always lands on a line boundary.
    consumed: u64,
    /// The usage records seen so far that are still inside the longest window.
    entries: Vec<Entry>,
    /// The model named by the most recent record, and when.
    model: Option<(DateTime<Utc>, String)>,
}

/// One usage record, reduced to what the windows need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Entry {
    at: DateTime<Utc>,
    tokens: u64,
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
            cache: Mutex::new(HashMap::new()),
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

    /// Brings the cache up to date and totals tokens per span.
    ///
    /// The expensive part of this provider is reading, and a transcript is
    /// append-only: what a refresh has already parsed cannot change. So each
    /// file is remembered by how many bytes of it have been consumed, and a
    /// refresh reads only what has been added since. Steady-state cost is
    /// therefore set by how much was written in the last thirty seconds rather
    /// than by how much history exists.
    fn scan(&self, projects: &Path, now: DateTime<Utc>) -> (Vec<u64>, Option<String>, ScanReport) {
        let mut report = ScanReport::default();
        let cutoff = now
            .checked_sub_signed(
                chrono::TimeDelta::from_std(self.longest_span()).unwrap_or(chrono::TimeDelta::MAX),
            )
            .unwrap_or(now);

        let mut cache = match self.cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let paths = transcripts(projects);
        report.files_seen = paths.len();

        for path in &paths {
            let Ok(length) = fs::metadata(path).map(|metadata| metadata.len()) else {
                // Deleted mid-scan, or not ours to read. Not a reason to report
                // no usage at all.
                continue;
            };

            let known = cache.contains_key(path);
            if !known && self.filter_by_mtime && !modified_since(path, cutoff) {
                // Append-only, so a file last touched before the longest window
                // cannot hold a record inside it. This is what keeps a machine
                // with months of history from paying for all of it on startup.
                report.files_skipped += 1;
                continue;
            }

            let state = cache.entry(path.clone()).or_default();

            // A file shorter than what was consumed from it was replaced or
            // truncated, so nothing remembered about it can be trusted.
            if length < state.consumed {
                *state = FileState::default();
            }

            if length > state.consumed {
                // Counted only when something was actually consumed. A file can
                // have grown and still yield nothing: the session being written
                // right now always ends in a partial line, and calling that a
                // read would report work that did not happen.
                if read_appended(path, state, &mut report) > 0 {
                    report.files_read += 1;
                }
            }
        }

        // A transcript that has gone leaves with its records; keeping them would
        // report usage from a session the user deleted.
        cache.retain(|path, _| paths.contains(path));

        let mut totals = vec![0_u64; self.spans.len()];
        let mut newest: Option<(DateTime<Utc>, String)> = None;

        for state in cache.values_mut() {
            // Records outside every window will never count again, and holding
            // them would grow the cache for as long as the process runs.
            state.entries.retain(|entry| entry.at >= cutoff);
            report.records_retained += state.entries.len();

            for entry in &state.entries {
                for (index, span) in self.spans.iter().enumerate() {
                    if within(entry.at, now, span.length)
                        && let Some(total) = totals.get_mut(index)
                    {
                        *total += entry.tokens;
                    }
                }
            }

            if let Some((at, model)) = &state.model
                && newest.as_ref().is_none_or(|(seen, _)| at > seen)
            {
                newest = Some((*at, model.clone()));
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
                report.files_read,
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

/// Parses whatever has been appended to a transcript since it was last read,
/// returning how many bytes were consumed.
///
/// Stops at the last complete line. A transcript being written to while it is
/// read ends mid-record, and consuming that partial line would both lose the
/// record and leave the offset in the middle of the next one — so the remainder
/// is left for the next refresh, by which time the writer will have finished it.
fn read_appended(path: &Path, state: &mut FileState, report: &mut ScanReport) -> u64 {
    let Ok(file) = fs::File::open(path) else {
        return 0;
    };

    let mut reader = BufReader::new(file);
    if state.consumed > 0 && reader.seek(SeekFrom::Start(state.consumed)).is_err() {
        return 0;
    }

    let mut consumed = 0;

    let mut line = Vec::new();
    loop {
        line.clear();
        let Ok(bytes) = reader.read_until(b'\n', &mut line) else {
            // An unreadable byte range. Stop rather than guess at where the next
            // line begins; the next refresh will retry from the same offset.
            report.unparseable_lines += 1;
            return consumed;
        };

        if bytes == 0 || !line.ends_with(b"\n") {
            return consumed;
        }

        let bytes = u64::try_from(bytes).unwrap_or_default();
        state.consumed += bytes;
        report.bytes_read += bytes;
        consumed += bytes;

        // Most of a transcript is prompts, responses, and file snapshots.
        // Deserialising all of it to find the token counters would be parsing
        // tens of megabytes to use a few kilobytes.
        let text = String::from_utf8_lossy(&line);
        if !text.contains("\"usage\"") {
            continue;
        }

        let Ok(record) = serde_json::from_str::<serde_json::Value>(&text) else {
            report.unparseable_lines += 1;
            continue;
        };

        let Some((at, tokens, model)) = read_record(&record) else {
            continue;
        };

        report.records_added += 1;
        state.entries.push(Entry { at, tokens });

        if let Some(model) = model
            && state.model.as_ref().is_none_or(|(seen, _)| at > *seen)
        {
            state.model = Some((at, model));
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
        assert_eq!(report.records_added, 1);
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

    /// A transcript that is still being written to, for the incremental tests.
    struct Transcript {
        home: tempfile::TempDir,
        path: PathBuf,
    }

    impl Transcript {
        fn new() -> Self {
            let home = tempfile::tempdir().unwrap();
            let directory = home.path().join("projects/-example-project");
            std::fs::create_dir_all(&directory).unwrap();
            let path = directory.join("session.jsonl");
            std::fs::write(&path, "").unwrap();
            Self { home, path }
        }

        /// Appends one usage record worth the given tokens.
        fn append(&self, at: &str, tokens: u64) {
            let record = format!(
                r#"{{"type":"assistant","timestamp":"{at}","message":{{"model":"claude-opus-5","usage":{{"input_tokens":{tokens},"output_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":9999}}}}}}"#
            );
            self.append_raw(&format!("{record}\n"));
        }

        /// Appends text exactly as given, newline included or not.
        fn append_raw(&self, text: &str) {
            use std::io::Write as _;
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&self.path)
                .unwrap();
            write!(file, "{text}").unwrap();
        }

        fn provider(&self) -> LocalFileProvider {
            LocalFileProvider::at(self.home.path()).with_now(instant(NOW))
        }
    }

    /// The point of the exercise: a refresh must cost what was *appended*, not
    /// what exists. At a 30-second interval, re-reading history is what blows
    /// the CPU budget.
    #[tokio::test]
    async fn a_refresh_reads_only_what_was_appended() {
        let transcript = Transcript::new();
        transcript.append("2026-08-01T11:00:00Z", 100);
        transcript.append("2026-08-01T11:10:00Z", 200);

        let provider = transcript.provider();
        provider.fetch().await.unwrap();
        let first = provider.last_scan().unwrap();
        assert!(first.bytes_read > 0);
        assert_eq!(first.records_added, 2);

        transcript.append("2026-08-01T11:20:00Z", 50);
        let snapshot = provider.fetch().await.unwrap();
        let second = provider.last_scan().unwrap();

        assert_eq!(
            second.records_added, 1,
            "the two records already parsed must not be parsed again"
        );
        assert!(
            second.bytes_read < first.bytes_read,
            "read {} bytes for one appended record after {} for two",
            second.bytes_read,
            first.bytes_read
        );
        assert_eq!(
            snapshot.window(WindowKind::Session).map(|w| w.used),
            Some(350),
            "and the total still counts every record"
        );
    }

    /// The steady state a tray actually lives in: nothing has changed since the
    /// last refresh, so a refresh should read nothing at all.
    #[tokio::test]
    async fn an_unchanged_transcript_is_not_read_again() {
        let transcript = Transcript::new();
        transcript.append("2026-08-01T11:00:00Z", 100);

        let provider = transcript.provider();
        provider.fetch().await.unwrap();

        for _ in 0..5 {
            let snapshot = provider.fetch().await.unwrap();
            let report = provider.last_scan().unwrap();

            assert_eq!(report.bytes_read, 0);
            assert_eq!(report.files_read, 0);
            assert_eq!(report.records_added, 0);
            assert_eq!(report.files_seen, 1, "the file is still stat-ed");
            assert_eq!(
                snapshot.window(WindowKind::Session).map(|w| w.used),
                Some(100),
                "the answer comes from what was already parsed"
            );
        }
    }

    /// A transcript is appended to while it is read. Consuming a half-written
    /// line would lose that record and leave the offset inside the next one, so
    /// every later reading would be wrong.
    #[tokio::test]
    async fn a_half_written_line_is_left_for_the_next_refresh() {
        let transcript = Transcript::new();
        transcript.append("2026-08-01T11:00:00Z", 100);
        transcript.append_raw(r#"{"type":"assistant","timestamp":"2026-08-01T11:3"#);

        let provider = transcript.provider();
        let snapshot = provider.fetch().await.unwrap();

        assert_eq!(
            snapshot.window(WindowKind::Session).map(|w| w.used),
            Some(100)
        );
        assert_eq!(
            provider.last_scan().unwrap().unparseable_lines,
            0,
            "an unfinished line is not a damaged one"
        );

        // The writer finishes the record it was in the middle of.
        transcript.append_raw(
            "0:00Z\",\"message\":{\"usage\":{\"input_tokens\":7,\"output_tokens\":0}}}\n",
        );
        let snapshot = provider.fetch().await.unwrap();

        assert_eq!(
            snapshot.window(WindowKind::Session).map(|w| w.used),
            Some(107),
            "the record completed after the first read still counts"
        );
    }

    /// A file that shrank was replaced, not appended to, so nothing remembered
    /// about it can be trusted.
    #[tokio::test]
    async fn a_truncated_transcript_is_re_read_from_the_start() {
        let transcript = Transcript::new();
        transcript.append("2026-08-01T11:00:00Z", 100);
        transcript.append("2026-08-01T11:05:00Z", 100);

        let provider = transcript.provider();
        assert_eq!(
            provider
                .fetch()
                .await
                .unwrap()
                .window(WindowKind::Session)
                .map(|w| w.used),
            Some(200)
        );

        std::fs::write(&transcript.path, "").unwrap();
        transcript.append("2026-08-01T11:10:00Z", 42);

        assert_eq!(
            provider
                .fetch()
                .await
                .unwrap()
                .window(WindowKind::Session)
                .map(|w| w.used),
            Some(42),
            "the replaced content must not be counted alongside the new"
        );
    }

    /// Deleting a session should stop it counting, or the tray would report
    /// usage from a transcript that no longer exists.
    #[tokio::test]
    async fn a_deleted_transcript_stops_counting() {
        let transcript = Transcript::new();
        transcript.append("2026-08-01T11:00:00Z", 100);

        let provider = transcript.provider();
        provider.fetch().await.unwrap();

        std::fs::remove_file(&transcript.path).unwrap();
        let snapshot = provider.fetch().await.unwrap();

        assert_eq!(
            snapshot.window(WindowKind::Session).map(|w| w.used),
            Some(0)
        );
        assert_eq!(provider.last_scan().unwrap().records_retained, 0);
    }

    /// Records that have aged out of every window are dropped, or the cache
    /// would grow for as long as the process runs.
    #[tokio::test]
    async fn records_outside_every_window_are_forgotten() {
        let transcript = Transcript::new();
        transcript.append("2026-05-01T11:00:00Z", 500);
        transcript.append("2026-08-01T11:00:00Z", 100);

        let provider = transcript.provider();
        let snapshot = provider.fetch().await.unwrap();

        assert_eq!(
            snapshot.window(WindowKind::Monthly).map(|w| w.used),
            Some(100)
        );
        assert_eq!(
            provider.last_scan().unwrap().records_retained,
            1,
            "the record from three months ago is held by no window"
        );
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
