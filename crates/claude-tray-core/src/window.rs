//! Quota windows — the rolling periods a plan's limits are measured over.

use core::fmt;
use core::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::Percent;

/// Which quota period a window measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    /// The rolling usage window that refills a few hours after it starts.
    Session,
    /// A weekly allowance.
    Weekly,
    /// A monthly allowance.
    Monthly,
}

impl WindowKind {
    /// Returns the wire name — lowercase, stable, and what gets persisted.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        }
    }
}

impl fmt::Display for WindowKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Session => "Session",
            Self::Weekly => "Weekly",
            Self::Monthly => "Monthly",
        };
        f.write_str(label)
    }
}

/// One quota window: how much of an allowance has been consumed, and when it
/// refills.
///
/// `used` and `limit` are deliberately unitless counts. Different sources
/// measure different things — tokens, messages, credits — and the domain model
/// only ever needs their ratio. A source that reports nothing but a percentage
/// can set `limit` to 100.
///
/// Both `limit` and `resets_at` are optional because a source may know that a
/// window exists without knowing its size or its refill time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageWindow {
    /// The period this window measures.
    pub kind: WindowKind,
    /// How much of the allowance has been consumed.
    pub used: u64,
    /// The size of the allowance, if known.
    pub limit: Option<u64>,
    /// When the allowance refills, if known.
    pub resets_at: Option<DateTime<Utc>>,
}

impl UsageWindow {
    /// Creates a window with a known limit and reset time.
    #[must_use]
    pub fn new(kind: WindowKind, used: u64, limit: u64, resets_at: DateTime<Utc>) -> Self {
        Self {
            kind,
            used,
            limit: Some(limit),
            resets_at: Some(resets_at),
        }
    }

    /// Creates a window whose limit and reset time the source did not report.
    ///
    /// Such a window contributes nothing to the remaining-percentage
    /// calculation; it exists so the tooltip can still say the window is there.
    #[must_use]
    pub fn unbounded(kind: WindowKind, used: u64) -> Self {
        Self {
            kind,
            used,
            limit: None,
            resets_at: None,
        }
    }

    /// Sets the reset time, replacing any existing one.
    #[must_use]
    pub fn with_reset(mut self, resets_at: DateTime<Utc>) -> Self {
        self.resets_at = Some(resets_at);
        self
    }

    /// Returns the allowance size, if the source reported a usable one.
    ///
    /// A limit of zero is treated as unknown rather than as "instantly
    /// exhausted": no real plan has a zero allowance, so a zero far more likely
    /// means the source had nothing to report.
    #[must_use]
    pub fn known_limit(&self) -> Option<u64> {
        self.limit.filter(|&limit| limit > 0)
    }

    /// Whether this window carries a usable limit.
    #[must_use]
    pub fn is_measurable(&self) -> bool {
        self.known_limit().is_some()
    }

    /// Returns how much of the allowance is left, as a percentage.
    ///
    /// `None` for a window with no usable limit — there is nothing to take a
    /// percentage of. Usage beyond the limit reads as zero remaining rather
    /// than as a negative: providers do report overage, and "less than empty"
    /// is not something the icon can draw.
    #[must_use]
    pub fn remaining_percent(&self) -> Option<Percent> {
        let limit = self.known_limit()?;
        let remaining = limit.saturating_sub(self.used);

        // Both are tallies of tokens or messages, orders of magnitude below the
        // 2^53 point where `f64` stops representing integers exactly.
        #[allow(clippy::cast_precision_loss)]
        let ratio = remaining as f64 / limit as f64;

        Percent::saturating(ratio * 100.0).ok()
    }

    /// Returns how long until the allowance refills.
    ///
    /// `None` if the source did not report a reset time. A reset time that has
    /// already passed reads as zero rather than as a negative duration — the
    /// refill is due, and the next snapshot will say so.
    #[must_use]
    pub fn time_until_reset(&self, now: DateTime<Utc>) -> Option<Duration> {
        let resets_at = self.resets_at?;
        Some(
            resets_at
                .signed_duration_since(now)
                .to_std()
                .unwrap_or(Duration::ZERO),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{UsageWindow, WindowKind};
    use crate::Percent;
    use chrono::{DateTime, Utc};
    use core::time::Duration;

    fn instant(text: &str) -> DateTime<Utc> {
        text.parse().unwrap()
    }

    #[test]
    fn a_bounded_window_is_measurable() {
        let window = UsageWindow::new(
            WindowKind::Session,
            40,
            200,
            instant("2026-08-01T12:00:00Z"),
        );
        assert!(window.is_measurable());
    }

    #[test]
    fn an_unbounded_window_is_not_measurable() {
        let window = UsageWindow::unbounded(WindowKind::Weekly, 40);
        assert!(!window.is_measurable());
        assert_eq!(window.resets_at, None);
    }

    /// A source reporting `limit: 0` means "I don't know", not "you have
    /// nothing left" — the latter would paint the icon red on bad data.
    #[test]
    fn a_zero_limit_counts_as_unknown() {
        let window = UsageWindow {
            kind: WindowKind::Monthly,
            used: 0,
            limit: Some(0),
            resets_at: None,
        };
        assert!(!window.is_measurable());
    }

    #[test]
    fn with_reset_attaches_a_refill_time() {
        let resets_at = instant("2026-08-01T18:30:00Z");
        let window = UsageWindow::unbounded(WindowKind::Session, 3).with_reset(resets_at);
        assert_eq!(window.resets_at, Some(resets_at));
    }

    #[test]
    fn round_trips_through_json() {
        let window = UsageWindow::new(
            WindowKind::Weekly,
            120,
            480,
            instant("2026-08-03T00:00:00Z"),
        );
        let json = serde_json::to_string(&window).unwrap();
        assert_eq!(serde_json::from_str::<UsageWindow>(&json).unwrap(), window);
    }

    #[test]
    fn remaining_percent_is_the_unused_share_of_the_limit() {
        let window = UsageWindow::new(
            WindowKind::Session,
            40,
            200,
            instant("2026-08-01T12:00:00Z"),
        );
        assert_eq!(window.remaining_percent(), Percent::new(80.0).ok());
    }

    #[test]
    fn an_unmeasurable_window_has_no_remaining_percent() {
        assert_eq!(
            UsageWindow::unbounded(WindowKind::Weekly, 40).remaining_percent(),
            None
        );
    }

    /// Providers do report usage past the limit. It reads as empty, not as a
    /// negative percentage the icon could not draw.
    #[test]
    fn overage_reads_as_empty_rather_than_negative() {
        let window = UsageWindow::new(
            WindowKind::Session,
            250,
            200,
            instant("2026-08-01T12:00:00Z"),
        );
        assert_eq!(window.remaining_percent(), Some(Percent::ZERO));
    }

    #[test]
    fn an_untouched_window_is_full() {
        let window = UsageWindow::new(WindowKind::Monthly, 0, 200, instant("2026-09-01T00:00:00Z"));
        assert_eq!(window.remaining_percent(), Some(Percent::FULL));
    }

    #[test]
    fn time_until_reset_counts_down_to_the_refill() {
        let window = UsageWindow::new(WindowKind::Session, 1, 10, instant("2026-08-01T12:30:00Z"));
        assert_eq!(
            window.time_until_reset(instant("2026-08-01T10:42:00Z")),
            Some(Duration::from_mins(108)),
        );
    }

    /// A reset that is already due counts as zero, not as a negative duration.
    #[test]
    fn a_passed_reset_time_reads_as_zero() {
        let window = UsageWindow::new(WindowKind::Session, 1, 10, instant("2026-08-01T09:00:00Z"));
        assert_eq!(
            window.time_until_reset(instant("2026-08-01T10:42:00Z")),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn a_window_without_a_reset_time_has_no_countdown() {
        assert_eq!(
            UsageWindow::unbounded(WindowKind::Weekly, 40)
                .time_until_reset(instant("2026-08-01T10:42:00Z")),
            None
        );
    }

    #[test]
    fn kind_serialises_in_snake_case() {
        assert_eq!(
            serde_json::to_string(&WindowKind::Monthly).unwrap(),
            "\"monthly\""
        );
        assert_eq!(WindowKind::Session.to_string(), "Session");
    }
}
