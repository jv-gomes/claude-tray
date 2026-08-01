//! Quota windows — the rolling periods a plan's limits are measured over.

use core::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

    /// Whether this window carries a usable limit.
    ///
    /// A limit of zero is treated as unknown rather than as "instantly
    /// exhausted": no real plan has a zero allowance, so a zero far more likely
    /// means the source had nothing to report.
    #[must_use]
    pub fn is_measurable(&self) -> bool {
        self.limit.is_some_and(|limit| limit > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::{UsageWindow, WindowKind};
    use chrono::{DateTime, Utc};

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
    fn kind_serialises_in_snake_case() {
        assert_eq!(
            serde_json::to_string(&WindowKind::Monthly).unwrap(),
            "\"monthly\""
        );
        assert_eq!(WindowKind::Session.to_string(), "Session");
    }
}
