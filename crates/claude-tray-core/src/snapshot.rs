//! A single reading of Claude usage at a point in time.

use core::fmt;
use core::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{Percent, Plan, UsageWindow, WindowKind};

/// Identifies which provider produced a snapshot.
///
/// Kept as a string rather than an enum so that the plugin boundary in phase 21
/// does not require a change to the domain model, and so a persisted history
/// entry from a provider that no longer exists still deserialises.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Source(String);

impl Source {
    /// Names a provider. The id should be lowercase and stable across releases,
    /// because it is persisted with every snapshot.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the provider id.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Everything one provider knew about usage at one moment.
///
/// A snapshot is immutable and self-contained: it carries the instant it was
/// captured rather than relying on when it is read, which is what makes
/// staleness and quota-reset detection decidable from a pair of snapshots
/// alone, with no clock in this crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageSnapshot {
    /// When this reading was taken, in UTC. Never the time it was displayed.
    pub captured_at: DateTime<Utc>,
    /// The subscription the reading belongs to, if the provider knows it.
    ///
    /// Optional because knowing what was consumed and knowing which plan it was
    /// consumed against are separate facts, and a source can have one without
    /// the other — local usage records say nothing about the subscription.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<Plan>,
    /// The quota windows the provider reported, in no particular order.
    pub windows: Vec<UsageWindow>,
    /// The model in use, if the provider reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Which provider produced this reading.
    pub source: Source,
}

impl UsageSnapshot {
    /// Creates a snapshot of what a provider measured.
    ///
    /// The plan and the model are attached separately, because they describe
    /// the account rather than the measurement and not every source reports
    /// them.
    #[must_use]
    pub fn new(captured_at: DateTime<Utc>, windows: Vec<UsageWindow>, source: Source) -> Self {
        Self {
            captured_at,
            plan: None,
            windows,
            model: None,
            source,
        }
    }

    /// Attaches the subscription the reading belongs to.
    #[must_use]
    pub fn with_plan(mut self, plan: Plan) -> Self {
        self.plan = Some(plan);
        self
    }

    /// Attaches the model name the reading applies to.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Returns the window of the given kind, if the provider reported one.
    ///
    /// If a provider reports the same kind twice — which would be a fault on
    /// its side — the first is returned.
    #[must_use]
    pub fn window(&self, kind: WindowKind) -> Option<&UsageWindow> {
        self.windows.iter().find(|window| window.kind == kind)
    }

    /// Returns the windows that carry a usable limit, and so can contribute to
    /// a remaining-usage figure.
    pub fn measurable_windows(&self) -> impl Iterator<Item = &UsageWindow> {
        self.windows.iter().filter(|window| window.is_measurable())
    }

    /// Returns the window with the least left in it.
    ///
    /// This is the window that decides what the user actually experiences: a
    /// session that is 90% free is no help when the weekly allowance is spent.
    /// Ties go to the first window the provider reported.
    #[must_use]
    pub fn most_constrained_window(&self) -> Option<&UsageWindow> {
        self.measurable_windows()
            .min_by_key(|window| window.remaining_percent())
    }

    /// Returns how much usage is left, taken from the most constrained window.
    ///
    /// `None` when no window carries a usable limit — the tray then has a
    /// snapshot but no number to draw, which is a different situation from
    /// having no snapshot at all.
    #[must_use]
    pub fn remaining_percent(&self) -> Option<Percent> {
        self.most_constrained_window()?.remaining_percent()
    }

    /// Returns how long until the most constrained window refills.
    ///
    /// Deliberately the same window that [`Self::remaining_percent`] reports
    /// on, so the tooltip's percentage and countdown describe one thing.
    #[must_use]
    pub fn time_until_reset(&self, now: DateTime<Utc>) -> Option<Duration> {
        self.most_constrained_window()?.time_until_reset(now)
    }

    /// Returns how long ago this reading was taken.
    ///
    /// A snapshot dated in the future — clock skew between this machine and the
    /// source — reads as brand new rather than as a negative age.
    #[must_use]
    pub fn age(&self, now: DateTime<Utc>) -> Duration {
        now.signed_duration_since(self.captured_at)
            .to_std()
            .unwrap_or(Duration::ZERO)
    }

    /// Whether this reading is too old to present as current.
    #[must_use]
    pub fn is_stale(&self, now: DateTime<Utc>, max_age: Duration) -> bool {
        self.age(now) > max_age
    }
}

#[cfg(test)]
mod tests {
    use super::{Source, UsageSnapshot};
    use crate::{Percent, Plan, UsageWindow, WindowKind};
    use chrono::{DateTime, Utc};
    use core::time::Duration;

    fn instant(text: &str) -> DateTime<Utc> {
        text.parse().unwrap()
    }

    fn snapshot() -> UsageSnapshot {
        UsageSnapshot::new(
            instant("2026-08-01T10:42:30Z"),
            vec![
                UsageWindow::new(
                    WindowKind::Session,
                    40,
                    200,
                    instant("2026-08-01T12:30:00Z"),
                ),
                UsageWindow::unbounded(WindowKind::Weekly, 900),
            ],
            Source::new("mock"),
        )
        .with_plan(Plan::Pro)
        .with_model("claude-opus-5")
    }

    #[test]
    fn finds_a_window_by_kind() {
        let snapshot = snapshot();
        assert_eq!(
            snapshot.window(WindowKind::Session).map(|w| w.used),
            Some(40)
        );
        assert_eq!(snapshot.window(WindowKind::Monthly), None);
    }

    #[test]
    fn measurable_windows_skip_the_ones_without_limits() {
        let kinds: Vec<_> = snapshot()
            .measurable_windows()
            .map(|window| window.kind)
            .collect();
        assert_eq!(kinds, vec![WindowKind::Session]);
    }

    #[test]
    fn a_snapshot_with_no_windows_has_nothing_measurable() {
        let snapshot = UsageSnapshot::new(
            instant("2026-08-01T10:42:30Z"),
            Vec::new(),
            Source::new("mock"),
        )
        .with_plan(Plan::Free);
        assert_eq!(snapshot.measurable_windows().count(), 0);
        assert_eq!(snapshot.model, None);
    }

    /// The window with the least left decides what the user experiences — a
    /// nearly-free session is no help once the weekly allowance is spent.
    #[test]
    fn the_most_constrained_window_wins() {
        let snapshot = UsageSnapshot::new(
            instant("2026-08-01T10:42:30Z"),
            vec![
                UsageWindow::new(
                    WindowKind::Session,
                    10,
                    100,
                    instant("2026-08-01T12:30:00Z"),
                ),
                UsageWindow::new(WindowKind::Weekly, 95, 100, instant("2026-08-03T00:00:00Z")),
            ],
            Source::new("mock"),
        )
        .with_plan(Plan::Max);

        assert_eq!(
            snapshot.most_constrained_window().map(|w| w.kind),
            Some(WindowKind::Weekly)
        );
        assert_eq!(snapshot.remaining_percent(), Percent::new(5.0).ok());
    }

    /// The countdown must describe the same window as the percentage, or the
    /// tooltip would be reporting on two different things.
    #[test]
    fn the_countdown_follows_the_same_window_as_the_percentage() {
        let snapshot = UsageSnapshot::new(
            instant("2026-08-01T10:42:30Z"),
            vec![
                UsageWindow::new(
                    WindowKind::Session,
                    10,
                    100,
                    instant("2026-08-01T11:00:00Z"),
                ),
                UsageWindow::new(WindowKind::Weekly, 95, 100, instant("2026-08-01T12:00:00Z")),
            ],
            Source::new("mock"),
        )
        .with_plan(Plan::Max);

        assert_eq!(
            snapshot.time_until_reset(instant("2026-08-01T10:42:30Z")),
            Some(Duration::from_secs(77 * 60 + 30)),
            "the weekly window is the constrained one, so its reset is the one shown"
        );
    }

    #[test]
    fn a_snapshot_with_no_measurable_window_has_no_percentage() {
        let snapshot = UsageSnapshot::new(
            instant("2026-08-01T10:42:30Z"),
            vec![UsageWindow::unbounded(WindowKind::Session, 3)],
            Source::new("mock"),
        )
        .with_plan(Plan::Free);
        assert_eq!(snapshot.most_constrained_window(), None);
        assert_eq!(snapshot.remaining_percent(), None);
        assert_eq!(
            snapshot.time_until_reset(instant("2026-08-01T10:42:30Z")),
            None
        );
    }

    #[test]
    fn staleness_is_measured_against_a_supplied_now() {
        let snapshot = snapshot();
        let max_age = Duration::from_secs(90);

        assert!(!snapshot.is_stale(instant("2026-08-01T10:43:00Z"), max_age));
        assert!(
            !snapshot.is_stale(instant("2026-08-01T10:44:00Z"), max_age),
            "exactly at the limit is not yet stale"
        );
        assert!(snapshot.is_stale(instant("2026-08-01T10:44:01Z"), max_age));
    }

    /// If this machine's clock is behind the source's, the reading is not
    /// negative-aged — it is simply new.
    #[test]
    fn a_future_dated_snapshot_is_not_stale() {
        let snapshot = snapshot();
        let earlier = instant("2026-08-01T10:00:00Z");

        assert_eq!(snapshot.age(earlier), Duration::ZERO);
        assert!(!snapshot.is_stale(earlier, Duration::from_secs(90)));
    }

    #[test]
    fn round_trips_through_json() {
        let snapshot = snapshot();
        let json = serde_json::to_string(&snapshot).unwrap();
        assert_eq!(
            serde_json::from_str::<UsageSnapshot>(&json).unwrap(),
            snapshot
        );
    }

    /// Timestamps are stored in UTC, so a snapshot means the same thing
    /// wherever it is read back.
    #[test]
    fn timestamps_persist_as_utc() {
        let json = serde_json::to_string(&snapshot()).unwrap();
        assert!(
            json.contains("\"2026-08-01T10:42:30Z\""),
            "captured_at should serialise as UTC: {json}"
        );
    }
}
