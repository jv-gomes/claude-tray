//! A single reading of Claude usage at a point in time.

use core::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{Plan, UsageWindow, WindowKind};

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
    /// The subscription the reading belongs to.
    pub plan: Plan,
    /// The quota windows the provider reported, in no particular order.
    pub windows: Vec<UsageWindow>,
    /// The model in use, if the provider reports one.
    pub model: Option<String>,
    /// Which provider produced this reading.
    pub source: Source,
}

impl UsageSnapshot {
    /// Creates a snapshot.
    #[must_use]
    pub fn new(
        captured_at: DateTime<Utc>,
        plan: Plan,
        windows: Vec<UsageWindow>,
        source: Source,
    ) -> Self {
        Self {
            captured_at,
            plan,
            windows,
            model: None,
            source,
        }
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
}

#[cfg(test)]
mod tests {
    use super::{Source, UsageSnapshot};
    use crate::{Plan, UsageWindow, WindowKind};
    use chrono::{DateTime, Utc};

    fn instant(text: &str) -> DateTime<Utc> {
        text.parse().unwrap()
    }

    fn snapshot() -> UsageSnapshot {
        UsageSnapshot::new(
            instant("2026-08-01T10:42:30Z"),
            Plan::Pro,
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
            Plan::Free,
            Vec::new(),
            Source::new("mock"),
        );
        assert_eq!(snapshot.measurable_windows().count(), 0);
        assert_eq!(snapshot.model, None);
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
