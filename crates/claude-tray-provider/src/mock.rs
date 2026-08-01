//! A provider that makes usage data up.

use core::sync::atomic::{AtomicUsize, Ordering};
use core::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use claude_tray_core::{Percent, Plan, Source, UsageSnapshot, UsageWindow, WindowKind};

use crate::{Health, ProviderCapabilities, ProviderError, UsageProvider};

/// A failure a source can produce, and the mock can be told to.
///
/// These are the four cases from the roadmap plus a 5xx, chosen because they
/// are the ones with genuinely different handling: one is the user's problem,
/// one resolves itself, one has a delay attached, and one means the source
/// changed shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The request never came back.
    Timeout,
    /// The credentials were rejected — an HTTP 401.
    Unauthorized,
    /// The source asked us to slow down — an HTTP 429.
    RateLimited {
        /// How long the source said to wait, if it said.
        retry_after: Option<Duration>,
    },
    /// The source is reachable but not serving usage — an HTTP 5xx.
    Unavailable,
    /// A response arrived in a shape the parser did not recognise.
    Malformed,
}

impl Fault {
    /// Builds the error this fault produces.
    fn into_error(self) -> ProviderError {
        match self {
            Self::Timeout => ProviderError::network_unavailable("mock: the request timed out"),
            Self::Unauthorized => {
                ProviderError::not_authenticated("mock: the credentials were rejected (401)")
            }
            Self::RateLimited { retry_after } => {
                ProviderError::rate_limited("mock: rate limited (429)", retry_after)
            }
            Self::Unavailable => {
                ProviderError::provider_unavailable("mock: the source is unavailable (503)")
            }
            Self::Malformed => {
                ProviderError::parse_failed("mock: the response was not in a recognised shape")
            }
        }
    }
}

/// One entry in a mock provider's script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Report this much usage remaining.
    Remaining(Percent),
    /// Fail this way.
    Fail(Fault),
}

impl Step {
    /// A step reporting the given percentage remaining.
    ///
    /// # Errors
    ///
    /// Returns [`PercentError`](claude_tray_core::PercentError) if the figure
    /// is not a percentage.
    pub fn remaining(percent: f64) -> Result<Self, claude_tray_core::PercentError> {
        Percent::new(percent).map(Self::Remaining)
    }
}

/// A provider that returns invented usage data.
///
/// This exists so that the icon, tooltip, menu, notifications, and scheduler can
/// be built and tested before it is known where real data comes from — the
/// reason ADR-0002 puts a trait in front of every source. It is also the only
/// way to exercise the error paths on demand: a 429 from a real source arrives
/// when it arrives, but [`Fault::RateLimited`] arrives when the test asks.
///
/// A fixed provider repeats one reading forever:
///
/// ```
/// # use claude_tray_provider::{MockProvider, UsageProvider};
/// # use claude_tray_core::Percent;
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = MockProvider::fixed(Percent::new(81.0)?);
/// let snapshot = provider.fetch().await?;
///
/// assert_eq!(snapshot.remaining_percent(), Percent::new(81.0).ok());
/// assert_eq!(snapshot.source.as_str(), "mock");
/// # Ok(())
/// # }
/// ```
///
/// A scripted one walks a sequence, one step per fetch, holding the last step
/// once the script runs out — or cycling, with [`MockProvider::repeating`].
#[derive(Debug)]
pub struct MockProvider {
    id: String,
    plan: Plan,
    model: Option<String>,
    steps: Vec<Step>,
    repeat: bool,
    resets_in: Duration,
    captured_at: Option<DateTime<Utc>>,
    cursor: AtomicUsize,
}

impl MockProvider {
    /// The window size the mock reports usage against.
    ///
    /// Large enough that a percentage survives the round trip into whole units
    /// and back to two decimal places, which keeps `--simulate-percent` honest.
    const LIMIT: u64 = 10_000;

    /// How long until the mock's window refills, unless told otherwise. The
    /// figure from the tooltip example in the README.
    const DEFAULT_RESETS_IN: Duration = Duration::from_mins(108);

    /// Creates a provider that reports the same figure on every fetch.
    #[must_use]
    pub fn fixed(remaining: Percent) -> Self {
        Self::scripted([Step::Remaining(remaining)])
    }

    /// Creates a provider that fails the same way on every fetch.
    #[must_use]
    pub fn failing(fault: Fault) -> Self {
        Self::scripted([Step::Fail(fault)])
    }

    /// Creates a provider that walks a script, one step per fetch.
    ///
    /// Once the script runs out the last step repeats, so a test that fetches
    /// more times than it scripted sees a steady state rather than a surprise.
    #[must_use]
    pub fn scripted(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            id: "mock".to_owned(),
            plan: Plan::Pro,
            model: Some("claude-opus-5".to_owned()),
            steps: steps.into_iter().collect(),
            repeat: false,
            resets_in: Self::DEFAULT_RESETS_IN,
            captured_at: None,
            cursor: AtomicUsize::new(0),
        }
    }

    /// Makes the script start over instead of holding its last step.
    #[must_use]
    pub fn repeating(mut self) -> Self {
        self.repeat = true;
        self
    }

    /// Renames the provider, so a fallback chain can hold several mocks and
    /// still be told apart by [`UsageSnapshot::source`].
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Sets the plan the snapshots claim.
    #[must_use]
    pub fn with_plan(mut self, plan: Plan) -> Self {
        self.plan = plan;
        self
    }

    /// Sets the model the snapshots claim, or drops it with `None` to mimic a
    /// source that does not report one.
    #[must_use]
    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model = model;
        self
    }

    /// Sets how long until the reported window refills.
    #[must_use]
    pub fn with_reset_in(mut self, resets_in: Duration) -> Self {
        self.resets_in = resets_in;
        self
    }

    /// Pins the capture time, so a test gets the same snapshot every run.
    ///
    /// Without this the mock reads the clock, which is correct for a provider —
    /// they are the I/O boundary — but no use to a test comparing timestamps.
    #[must_use]
    pub fn with_captured_at(mut self, captured_at: DateTime<Utc>) -> Self {
        self.captured_at = Some(captured_at);
        self
    }

    /// Returns the step this fetch should act on, advancing the cursor.
    fn advance(&self) -> Option<Step> {
        let position = self.cursor.fetch_add(1, Ordering::Relaxed);
        self.step_at(position)
    }

    /// Returns the step at a position, without advancing.
    fn step_at(&self, position: usize) -> Option<Step> {
        if self.steps.is_empty() {
            return None;
        }
        let index = if self.repeat {
            position % self.steps.len()
        } else {
            position.min(self.steps.len() - 1)
        };
        self.steps.get(index).copied()
    }

    /// Builds a snapshot reporting the given figure.
    fn snapshot(&self, remaining: Percent) -> UsageSnapshot {
        let captured_at = self.captured_at.unwrap_or_else(Utc::now);
        let resets_at = TimeDelta::from_std(self.resets_in)
            .ok()
            .and_then(|delta| captured_at.checked_add_signed(delta))
            .unwrap_or(captured_at);

        // The product is in `0..=LIMIT` by construction, so the cast neither
        // truncates nor loses a sign.
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let used = (((100.0 - remaining.value()) / 100.0) * Self::LIMIT as f64).round() as u64;

        let snapshot = UsageSnapshot::new(
            captured_at,
            vec![UsageWindow::new(
                WindowKind::Session,
                used,
                Self::LIMIT,
                resets_at,
            )],
            Source::new(self.id.clone()),
        )
        .with_plan(self.plan.clone());

        match &self.model {
            Some(model) => snapshot.with_model(model.clone()),
            None => snapshot,
        }
    }
}

impl Default for MockProvider {
    /// A provider reporting the 81% from the README's tooltip example.
    fn default() -> Self {
        Self::fixed(Percent::new(81.0).unwrap_or(Percent::FULL))
    }
}

#[async_trait]
impl UsageProvider for MockProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            windows: vec![WindowKind::Session],
            limits: true,
            reset_times: true,
            plan: true,
            model: self.model.is_some(),
        }
    }

    async fn fetch(&self) -> Result<UsageSnapshot, ProviderError> {
        match self.advance() {
            Some(Step::Remaining(remaining)) => Ok(self.snapshot(remaining)),
            Some(Step::Fail(fault)) => Err(fault.into_error()),
            None => Err(ProviderError::internal("mock: the script is empty")),
        }
    }

    /// Reports what the *next* fetch would do, without consuming it.
    async fn health(&self) -> Health {
        let position = self.cursor.load(Ordering::Relaxed);
        match self.step_at(position) {
            Some(Step::Remaining(_)) => Health::Ready,
            Some(Step::Fail(Fault::Unauthorized)) => Health::Unavailable {
                reason: "not signed in".to_owned(),
            },
            Some(Step::Fail(fault)) => Health::Degraded {
                reason: fault.into_error().message().to_owned(),
            },
            None => Health::Unavailable {
                reason: "the script is empty".to_owned(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Fault, MockProvider, Step};
    use crate::{Health, ProviderCapabilities, UsageProvider};
    use chrono::{DateTime, Utc};
    use claude_tray_core::{ErrorKind, Percent, Plan, WindowKind};
    use core::time::Duration;

    fn percent(value: f64) -> Percent {
        Percent::new(value).unwrap()
    }

    fn instant(text: &str) -> DateTime<Utc> {
        text.parse().unwrap()
    }

    #[tokio::test]
    async fn a_fixed_provider_reports_the_same_figure_forever() {
        let provider = MockProvider::fixed(percent(81.0));

        for _ in 0..5 {
            let snapshot = provider.fetch().await.unwrap();
            assert_eq!(snapshot.remaining_percent(), Some(percent(81.0)));
        }
    }

    #[tokio::test]
    async fn a_snapshot_carries_the_provider_that_made_it() {
        let provider = MockProvider::fixed(percent(50.0)).with_id("mock-secondary");
        let snapshot = provider.fetch().await.unwrap();

        assert_eq!(snapshot.source.as_str(), "mock-secondary");
        assert_eq!(provider.id(), "mock-secondary");
    }

    /// A requested figure has to come back out, or `--simulate-percent` would
    /// be a lie. The mock reports usage in whole units out of `LIMIT`, so the
    /// guarantee is one unit — a hundredth of a point.
    #[tokio::test]
    async fn a_requested_figure_survives_the_round_trip() {
        for value in [0.0, 4.99, 14.9, 33.33, 74.9, 81.5, 99.99, 100.0] {
            let snapshot = MockProvider::fixed(percent(value)).fetch().await.unwrap();
            let reported = snapshot.remaining_percent().unwrap().value();

            assert!(
                (reported - value).abs() <= 0.01,
                "asked for {value}, got {reported}"
            );
        }
    }

    /// The figures that decide a colour band or a battery segment must come
    /// back bit-exact, or a mock run would land on the wrong side of a boundary
    /// and the screenshot would be of the wrong state.
    #[tokio::test]
    async fn the_boundaries_come_back_exactly() {
        for value in [0.0, 5.0, 10.0, 15.0, 25.0, 40.0, 50.0, 75.0, 90.0, 100.0] {
            let snapshot = MockProvider::fixed(percent(value)).fetch().await.unwrap();

            assert_eq!(
                snapshot.remaining_percent(),
                Some(percent(value)),
                "{value} did not survive exactly"
            );
        }
    }

    #[tokio::test]
    async fn a_script_walks_one_step_per_fetch() {
        let provider = MockProvider::scripted([
            Step::Remaining(percent(80.0)),
            Step::Remaining(percent(50.0)),
            Step::Remaining(percent(20.0)),
        ]);

        let mut seen = Vec::new();
        for _ in 0..3 {
            seen.push(provider.fetch().await.unwrap().remaining_percent());
        }

        assert_eq!(
            seen,
            vec![
                Some(percent(80.0)),
                Some(percent(50.0)),
                Some(percent(20.0))
            ]
        );
    }

    /// Fetching past the end of a script gives a steady state rather than a
    /// surprise.
    #[tokio::test]
    async fn a_finished_script_holds_its_last_step() {
        let provider = MockProvider::scripted([
            Step::Remaining(percent(80.0)),
            Step::Remaining(percent(5.0)),
        ]);

        provider.fetch().await.unwrap();
        for _ in 0..3 {
            assert_eq!(
                provider.fetch().await.unwrap().remaining_percent(),
                Some(percent(5.0))
            );
        }
    }

    #[tokio::test]
    async fn a_repeating_script_starts_over() {
        let provider = MockProvider::scripted([
            Step::Remaining(percent(80.0)),
            Step::Remaining(percent(5.0)),
        ])
        .repeating();

        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push(provider.fetch().await.unwrap().remaining_percent());
        }

        assert_eq!(
            seen,
            vec![
                Some(percent(80.0)),
                Some(percent(5.0)),
                Some(percent(80.0)),
                Some(percent(5.0)),
            ]
        );
    }

    /// Each injected fault has to arrive as the kind the tray renders and with
    /// the retry policy the scheduler acts on.
    #[tokio::test]
    async fn faults_arrive_as_the_kinds_the_tray_renders() {
        let cases = [
            (Fault::Timeout, ErrorKind::NetworkUnavailable, true),
            (Fault::Unauthorized, ErrorKind::NotAuthenticated, false),
            (
                Fault::RateLimited { retry_after: None },
                ErrorKind::RateLimited,
                true,
            ),
            (Fault::Unavailable, ErrorKind::ProviderUnavailable, true),
            (Fault::Malformed, ErrorKind::ParseFailed, false),
        ];

        for (fault, kind, retryable) in cases {
            let error = MockProvider::failing(fault).fetch().await.unwrap_err();

            assert_eq!(error.kind(), kind, "{fault:?}");
            assert_eq!(error.is_retryable(), retryable, "{fault:?}");
        }
    }

    #[tokio::test]
    async fn a_rate_limit_can_carry_the_delay_the_source_asked_for() {
        let provider = MockProvider::failing(Fault::RateLimited {
            retry_after: Some(Duration::from_mins(2)),
        });

        let error = provider.fetch().await.unwrap_err();
        assert_eq!(error.retry_after(), Some(Duration::from_mins(2)));
    }

    /// The point of a script is to drive the state machine through a run that
    /// would take a real source hours to produce.
    #[tokio::test]
    async fn a_script_can_mix_readings_and_failures() {
        let provider = MockProvider::scripted([
            Step::Remaining(percent(60.0)),
            Step::Fail(Fault::Timeout),
            Step::Remaining(percent(45.0)),
        ]);

        assert!(provider.fetch().await.is_ok());
        assert_eq!(
            provider.fetch().await.unwrap_err().kind(),
            ErrorKind::NetworkUnavailable
        );
        assert_eq!(
            provider.fetch().await.unwrap().remaining_percent(),
            Some(percent(45.0))
        );
    }

    #[tokio::test]
    async fn an_empty_script_fails_rather_than_panicking() {
        let provider = MockProvider::scripted([]);

        assert_eq!(
            provider.fetch().await.unwrap_err().kind(),
            ErrorKind::Internal
        );
        assert!(!provider.health().await.is_usable());
    }

    #[tokio::test]
    async fn health_describes_the_next_fetch_without_consuming_it() {
        let provider = MockProvider::scripted([
            Step::Fail(Fault::Unauthorized),
            Step::Remaining(percent(80.0)),
        ]);

        assert_eq!(
            provider.health().await,
            Health::Unavailable {
                reason: "not signed in".to_owned()
            }
        );
        assert!(
            !provider.health().await.is_usable(),
            "asking twice must not have advanced the script"
        );

        provider.fetch().await.unwrap_err();
        assert_eq!(provider.health().await, Health::Ready);
    }

    #[tokio::test]
    async fn a_recoverable_fault_is_degraded_rather_than_unavailable() {
        let provider = MockProvider::failing(Fault::Timeout);

        assert!(
            provider.health().await.is_usable(),
            "a timeout is worth retrying, unlike a rejected credential"
        );
    }

    #[tokio::test]
    async fn a_pinned_capture_time_makes_snapshots_reproducible() {
        let captured_at = instant("2026-08-01T10:42:30Z");
        let provider = MockProvider::fixed(percent(81.0))
            .with_captured_at(captured_at)
            .with_reset_in(Duration::from_mins(108));

        let snapshot = provider.fetch().await.unwrap();

        assert_eq!(snapshot.captured_at, captured_at);
        assert_eq!(
            snapshot.time_until_reset(captured_at),
            Some(Duration::from_mins(108))
        );
        assert!(!snapshot.is_stale(captured_at, Duration::from_mins(1)));
    }

    #[tokio::test]
    async fn an_unpinned_provider_reads_the_clock() {
        let before = Utc::now();
        let snapshot = MockProvider::default().fetch().await.unwrap();

        assert!(snapshot.captured_at >= before);
        assert_eq!(snapshot.remaining_percent(), Some(percent(81.0)));
    }

    #[tokio::test]
    async fn the_plan_and_model_are_configurable() {
        let provider = MockProvider::fixed(percent(50.0))
            .with_plan(Plan::Max)
            .with_model(Some("claude-sonnet-5".to_owned()));

        let snapshot = provider.fetch().await.unwrap();

        assert_eq!(snapshot.plan, Some(Plan::Max));
        assert_eq!(snapshot.model.as_deref(), Some("claude-sonnet-5"));
    }

    /// Capabilities must describe what the provider does, not what it wishes it
    /// did — the tooltip hides rows on the strength of this.
    #[tokio::test]
    async fn capabilities_track_what_the_mock_actually_reports() {
        let provider = MockProvider::fixed(percent(50.0));
        let capabilities = provider.capabilities();

        assert!(capabilities.reports_percentage());
        assert!(capabilities.supports_window(WindowKind::Session));
        assert!(!capabilities.supports_window(WindowKind::Weekly));
        assert!(capabilities.model);

        let modelless = MockProvider::fixed(percent(50.0)).with_model(None);
        assert!(!modelless.capabilities().model);
    }

    /// The trait is the whole point: everything downstream holds one of these.
    #[tokio::test]
    async fn it_can_be_used_through_the_trait_object() {
        let providers: Vec<Box<dyn UsageProvider>> = vec![
            Box::new(MockProvider::failing(Fault::Unauthorized).with_id("first")),
            Box::new(MockProvider::fixed(percent(30.0)).with_id("second")),
        ];

        let mut sources = Vec::new();
        for provider in &providers {
            if let Ok(snapshot) = provider.fetch().await {
                sources.push(snapshot.source.as_str().to_owned());
            }
        }

        assert_eq!(sources, vec!["second".to_owned()]);
        assert_eq!(
            providers[1].capabilities(),
            ProviderCapabilities {
                windows: vec![WindowKind::Session],
                limits: true,
                reset_times: true,
                plan: true,
                model: true,
            }
        );
    }

    #[test]
    fn a_step_rejects_a_figure_that_is_not_a_percentage() {
        assert!(Step::remaining(50.0).is_ok());
        assert!(Step::remaining(101.0).is_err());
    }
}
