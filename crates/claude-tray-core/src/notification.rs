//! Deciding when the user is worth interrupting.

use core::time::Duration;

use chrono::{DateTime, Utc};

use crate::{Percent, UsageSnapshot, WindowKind};

/// A threshold the user wants to hear about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationRule {
    /// Fires as usage falls to or below this figure.
    pub threshold_percent: Percent,
    /// Whether the rule is active. A disabled rule is kept rather than removed
    /// so that turning it back on in the settings restores the same threshold.
    pub enabled: bool,
}

impl NotificationRule {
    /// The thresholds from the spec: 50%, 25%, 10%, and 5% remaining.
    pub const DEFAULT_THRESHOLDS: [Percent; 4] = [
        Percent::from_literal(50.0),
        Percent::from_literal(25.0),
        Percent::from_literal(10.0),
        Percent::from_literal(5.0),
    ];

    /// Creates an enabled rule.
    #[must_use]
    pub fn new(threshold_percent: Percent) -> Self {
        Self {
            threshold_percent,
            enabled: true,
        }
    }

    /// Creates a rule that is present but switched off.
    #[must_use]
    pub fn disabled(threshold_percent: Percent) -> Self {
        Self {
            threshold_percent,
            enabled: false,
        }
    }

    /// Returns the default set of rules, all enabled.
    #[must_use]
    pub fn defaults() -> Vec<Self> {
        Self::DEFAULT_THRESHOLDS.map(Self::new).to_vec()
    }
}

/// Something worth telling the user about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationEvent {
    /// Usage fell to or below a rule's threshold.
    ThresholdCrossed {
        /// The threshold that was crossed.
        threshold: Percent,
        /// What is actually left, which may be well below the threshold if
        /// usage moved a long way between two readings.
        remaining: Percent,
    },
    /// A quota window refilled.
    QuotaReset {
        /// Which window rolled over.
        kind: WindowKind,
        /// When the new window is due to refill.
        resets_at: DateTime<Utc>,
    },
}

/// Decides which notifications a run of snapshots deserves.
///
/// Edge-triggered: a rule fires on the reading that takes usage *down* to its
/// threshold, and not again until usage has gone back above it. Level-triggered
/// rules would re-fire on every refresh for as long as the figure stayed low,
/// which for a 30-second refresh is roughly two notifications a minute.
///
/// The engine holds no clock. It reads time from each snapshot's
/// `captured_at`, which is when the reading was actually taken — not when it
/// happened to be processed.
///
/// ```
/// use claude_tray_core::{NotificationEngine, NotificationEvent};
/// # use claude_tray_core::{Percent, Plan, Source, UsageSnapshot, UsageWindow, WindowKind};
/// # use chrono::{DateTime, Utc};
/// # fn at(text: &str, used: u64) -> UsageSnapshot {
/// #     UsageSnapshot::new(
/// #         text.parse::<DateTime<Utc>>().unwrap(),
/// #         Plan::Pro,
/// #         vec![UsageWindow::new(
/// #             WindowKind::Session,
/// #             used,
/// #             100,
/// #             "2026-08-01T18:00:00Z".parse().unwrap(),
/// #         )],
/// #         Source::new("mock"),
/// #     )
/// # }
/// let mut engine = NotificationEngine::default();
///
/// // The first reading is a starting point, never a crossing.
/// assert!(engine.observe(&at("2026-08-01T10:00:00Z", 40)).is_empty());
///
/// // Falling through 50% remaining fires once.
/// let events = engine.observe(&at("2026-08-01T10:00:30Z", 55));
/// assert_eq!(events.len(), 1);
///
/// // Staying there does not fire again.
/// assert!(engine.observe(&at("2026-08-01T10:01:00Z", 58)).is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct NotificationEngine {
    rules: Vec<TrackedRule>,
    cooldown: Duration,
    last: Option<Observation>,
}

/// A rule plus when it last fired.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TrackedRule {
    rule: NotificationRule,
    fired_at: Option<DateTime<Utc>>,
}

/// What the previous snapshot said, reduced to what the engine compares
/// against.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Observation {
    remaining: Option<Percent>,
    windows: Vec<WindowMark>,
}

/// When one window of the previous snapshot was due to refill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowMark {
    kind: WindowKind,
    resets_at: DateTime<Utc>,
}

impl NotificationEngine {
    /// How long a rule stays quiet after firing, unless a quota reset clears
    /// it: fifteen minutes, or thirty refreshes at the default interval.
    pub const DEFAULT_COOLDOWN: Duration = Duration::from_mins(15);

    /// Creates an engine with the given rules and cooldown.
    #[must_use]
    pub fn new(rules: Vec<NotificationRule>, cooldown: Duration) -> Self {
        Self {
            rules: rules.into_iter().map(TrackedRule::new).collect(),
            cooldown,
            last: None,
        }
    }

    /// Returns the rules as configured.
    #[must_use]
    pub fn rules(&self) -> Vec<NotificationRule> {
        self.rules.iter().map(|tracked| tracked.rule).collect()
    }

    /// Replaces the rules, forgetting which ones had fired.
    pub fn set_rules(&mut self, rules: Vec<NotificationRule>) {
        self.rules = rules.into_iter().map(TrackedRule::new).collect();
    }

    /// How long a rule stays quiet after firing.
    #[must_use]
    pub fn cooldown(&self) -> Duration {
        self.cooldown
    }

    /// Forgets everything observed so far.
    ///
    /// The next snapshot becomes a fresh starting point, so it cannot cross
    /// anything. Call this when the run of readings is broken — a different
    /// account, a different provider — where comparing against the last
    /// snapshot would be comparing two different things.
    pub fn reset(&mut self) {
        self.last = None;
        self.clear_fired();
    }

    /// Feeds in a snapshot and returns what the user should be told.
    ///
    /// The first snapshot after construction or [`Self::reset`] never produces
    /// a crossing: it establishes where usage stands, and starting the
    /// application at 4% remaining is not the same event as falling to 4%.
    pub fn observe(&mut self, snapshot: &UsageSnapshot) -> Vec<NotificationEvent> {
        let remaining = snapshot.remaining_percent();
        let windows = WindowMark::all_of(snapshot);
        let mut events = Vec::new();

        if let Some(last) = self.last.take() {
            events.extend(resets_between(&last.windows, &windows));

            if events.is_empty() {
                // A reset and a crossing in the same step would be measuring a
                // fall across two different windows, so only look for one when
                // nothing rolled over.
                events.extend(self.crossing(last.remaining, remaining, snapshot.captured_at));
            } else {
                // The quota refilled, so whatever the rules had already
                // announced no longer applies to the window they announced it
                // about.
                self.clear_fired();
            }
        }

        self.last = Some(Observation { remaining, windows });
        events
    }

    /// Finds the crossing, if any, that this pair of readings represents.
    fn crossing(
        &mut self,
        before: Option<Percent>,
        after: Option<Percent>,
        captured_at: DateTime<Utc>,
    ) -> Option<NotificationEvent> {
        let (before, after) = (before?, after?);
        let cooldown = self.cooldown;

        // Several thresholds can be crossed at once when usage moves a long way
        // between two readings. Only the most serious is worth saying: four
        // notifications arriving together tell the user one thing, four times.
        let crossed = self
            .rules
            .iter_mut()
            .filter(|tracked| tracked.rule.enabled)
            .filter(|tracked| {
                let threshold = tracked.rule.threshold_percent;
                before > threshold && after <= threshold
            })
            .min_by_key(|tracked| tracked.rule.threshold_percent)?;

        if crossed.is_cooling_down(captured_at, cooldown) {
            return None;
        }

        crossed.fired_at = Some(captured_at);
        Some(NotificationEvent::ThresholdCrossed {
            threshold: crossed.rule.threshold_percent,
            remaining: after,
        })
    }

    /// Re-arms every rule against the cooldown.
    fn clear_fired(&mut self) {
        for tracked in &mut self.rules {
            tracked.fired_at = None;
        }
    }
}

impl Default for NotificationEngine {
    fn default() -> Self {
        Self::new(NotificationRule::defaults(), Self::DEFAULT_COOLDOWN)
    }
}

impl TrackedRule {
    fn new(rule: NotificationRule) -> Self {
        Self {
            rule,
            fired_at: None,
        }
    }

    /// Whether this rule fired recently enough to stay quiet.
    ///
    /// A snapshot older than the last one it fired on — readings arriving out
    /// of order — counts as no time having passed, which keeps the rule quiet
    /// rather than letting a stale reading re-announce something.
    fn is_cooling_down(&self, now: DateTime<Utc>, cooldown: Duration) -> bool {
        self.fired_at.is_some_and(|fired_at| {
            let elapsed = now
                .signed_duration_since(fired_at)
                .to_std()
                .unwrap_or(Duration::ZERO);
            elapsed < cooldown
        })
    }
}

impl WindowMark {
    /// Collects the refill times a snapshot reported, one per window that has
    /// one.
    fn all_of(snapshot: &UsageSnapshot) -> Vec<Self> {
        snapshot
            .windows
            .iter()
            .filter_map(|window| {
                window.resets_at.map(|resets_at| Self {
                    kind: window.kind,
                    resets_at,
                })
            })
            .collect()
    }
}

/// Finds the windows that rolled over between two snapshots.
///
/// A window has reset when its refill time has moved later: the reading now
/// describes the *next* window rather than the one before it. Comparing usage
/// counts instead would misread a provider correcting a figure downwards as a
/// reset.
fn resets_between(before: &[WindowMark], after: &[WindowMark]) -> Vec<NotificationEvent> {
    after
        .iter()
        .filter(|new| {
            before
                .iter()
                .find(|old| old.kind == new.kind)
                .is_some_and(|old| new.resets_at > old.resets_at)
        })
        .map(|new| NotificationEvent::QuotaReset {
            kind: new.kind,
            resets_at: new.resets_at,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{NotificationEngine, NotificationEvent, NotificationRule};
    use crate::{Percent, Plan, Source, UsageSnapshot, UsageWindow, WindowKind};
    use chrono::{DateTime, Utc};
    use core::time::Duration;

    fn instant(text: &str) -> DateTime<Utc> {
        text.parse().unwrap()
    }

    fn percent(value: f64) -> Percent {
        Percent::new(value).unwrap()
    }

    /// A session snapshot with `remaining` percent left, out of a limit of 100.
    fn reading(captured_at: &str, remaining: u64, resets_at: &str) -> UsageSnapshot {
        UsageSnapshot::new(
            instant(captured_at),
            Plan::Pro,
            vec![UsageWindow::new(
                WindowKind::Session,
                100 - remaining,
                100,
                instant(resets_at),
            )],
            Source::new("mock"),
        )
    }

    const RESET: &str = "2026-08-01T18:00:00Z";

    fn crossed(threshold: f64, remaining: f64) -> NotificationEvent {
        NotificationEvent::ThresholdCrossed {
            threshold: percent(threshold),
            remaining: percent(remaining),
        }
    }

    /// Starting the application at 4% remaining is not the same event as
    /// falling to 4%.
    #[test]
    fn the_first_reading_never_fires() {
        let mut engine = NotificationEngine::default();
        assert!(
            engine
                .observe(&reading("2026-08-01T10:00:00Z", 4, RESET))
                .is_empty()
        );
    }

    /// P2-27: crossing down fires, and only once.
    #[test]
    fn crossing_down_fires_once() {
        let mut engine = NotificationEngine::default();

        engine.observe(&reading("2026-08-01T10:00:00Z", 60, RESET));

        assert_eq!(
            engine.observe(&reading("2026-08-01T10:00:30Z", 45, RESET)),
            vec![crossed(50.0, 45.0)]
        );
        assert!(
            engine
                .observe(&reading("2026-08-01T10:01:00Z", 42, RESET))
                .is_empty(),
            "still below the threshold, but not a new crossing"
        );
    }

    /// P2-27: crossing up does not fire.
    #[test]
    fn crossing_up_does_not_fire() {
        let mut engine = NotificationEngine::default();

        engine.observe(&reading("2026-08-01T10:00:00Z", 45, RESET));
        assert!(
            engine
                .observe(&reading("2026-08-01T10:00:30Z", 60, RESET))
                .is_empty()
        );
        assert!(
            engine
                .observe(&reading("2026-08-01T10:01:00Z", 80, RESET))
                .is_empty()
        );
    }

    /// P2-27: after a quota reset the same threshold fires again.
    #[test]
    fn re_crossing_after_a_reset_fires_again() {
        let mut engine = NotificationEngine::default();
        let later = "2026-08-01T23:00:00Z";

        engine.observe(&reading("2026-08-01T10:00:00Z", 60, RESET));
        assert_eq!(
            engine.observe(&reading("2026-08-01T10:00:30Z", 45, RESET)),
            vec![crossed(50.0, 45.0)]
        );

        // The window refills, then usage falls through 50% all over again.
        assert_eq!(
            engine.observe(&reading("2026-08-01T18:00:30Z", 100, later)),
            vec![NotificationEvent::QuotaReset {
                kind: WindowKind::Session,
                resets_at: instant(later),
            }]
        );
        assert_eq!(
            engine.observe(&reading("2026-08-01T18:01:00Z", 45, later)),
            vec![crossed(50.0, 45.0)],
            "a new window is a new chance to be warned about it"
        );
    }

    /// P2-31: a snapshot pair spanning a window boundary is what reveals the
    /// reset — the refill time has moved on.
    #[test]
    fn a_reset_is_detected_from_the_refill_time_moving_forward() {
        let mut engine = NotificationEngine::default();
        let later = "2026-08-02T00:00:00Z";

        engine.observe(&reading("2026-08-01T17:59:30Z", 8, RESET));

        assert_eq!(
            engine.observe(&reading("2026-08-01T18:00:30Z", 100, later)),
            vec![NotificationEvent::QuotaReset {
                kind: WindowKind::Session,
                resets_at: instant(later),
            }]
        );
    }

    /// The same refill time from one reading to the next is the same window,
    /// however much the usage figure moved.
    #[test]
    fn an_unchanged_refill_time_is_not_a_reset() {
        let mut engine = NotificationEngine::default();

        engine.observe(&reading("2026-08-01T10:00:00Z", 80, RESET));
        let events = engine.observe(&reading("2026-08-01T10:00:30Z", 60, RESET));

        assert!(
            !events
                .iter()
                .any(|event| matches!(event, NotificationEvent::QuotaReset { .. })),
            "{events:?}"
        );
    }

    #[test]
    fn each_window_resets_on_its_own_schedule() {
        let mut engine = NotificationEngine::default();
        let snapshot = |session: &str, weekly: &str| {
            UsageSnapshot::new(
                instant("2026-08-01T18:00:30Z"),
                Plan::Max,
                vec![
                    UsageWindow::new(WindowKind::Session, 10, 100, instant(session)),
                    UsageWindow::new(WindowKind::Weekly, 90, 100, instant(weekly)),
                ],
                Source::new("mock"),
            )
        };

        engine.observe(&snapshot("2026-08-01T18:00:00Z", "2026-08-03T00:00:00Z"));
        let events = engine.observe(&snapshot("2026-08-02T00:00:00Z", "2026-08-03T00:00:00Z"));

        assert_eq!(
            events,
            vec![NotificationEvent::QuotaReset {
                kind: WindowKind::Session,
                resets_at: instant("2026-08-02T00:00:00Z"),
            }],
            "the weekly window has not rolled over, so it says nothing"
        );
    }

    /// A figure resting on a threshold would otherwise announce itself twice a
    /// minute.
    #[test]
    fn a_flapping_figure_is_silenced_by_the_cooldown() {
        let mut engine = NotificationEngine::default();

        engine.observe(&reading("2026-08-01T10:00:00Z", 60, RESET));
        assert_eq!(
            engine.observe(&reading("2026-08-01T10:00:30Z", 49, RESET)),
            vec![crossed(50.0, 49.0)]
        );

        // Up over the line and back down, twice, all within the cooldown.
        for minute in 1..5 {
            engine.observe(&reading(&format!("2026-08-01T10:0{minute}:00Z"), 51, RESET));
            assert!(
                engine
                    .observe(&reading(&format!("2026-08-01T10:0{minute}:30Z"), 49, RESET))
                    .is_empty(),
                "minute {minute} re-announced a crossing the user was just told about"
            );
        }
    }

    #[test]
    fn the_cooldown_expires() {
        let mut engine =
            NotificationEngine::new(NotificationRule::defaults(), Duration::from_mins(5));

        engine.observe(&reading("2026-08-01T10:00:00Z", 60, RESET));
        engine.observe(&reading("2026-08-01T10:00:30Z", 49, RESET));

        engine.observe(&reading("2026-08-01T10:06:00Z", 51, RESET));
        assert_eq!(
            engine.observe(&reading("2026-08-01T10:06:30Z", 49, RESET)),
            vec![crossed(50.0, 49.0)]
        );
    }

    /// Four notifications arriving together tell the user one thing, four
    /// times.
    #[test]
    fn a_large_fall_announces_only_the_most_serious_threshold() {
        let mut engine = NotificationEngine::default();

        engine.observe(&reading("2026-08-01T10:00:00Z", 60, RESET));
        assert_eq!(
            engine.observe(&reading("2026-08-01T10:00:30Z", 3, RESET)),
            vec![crossed(5.0, 3.0)],
            "50, 25, 10 and 5 were all crossed; only 5 is worth saying"
        );
    }

    #[test]
    fn a_disabled_rule_stays_quiet() {
        let mut engine = NotificationEngine::new(
            vec![
                NotificationRule::disabled(percent(50.0)),
                NotificationRule::new(percent(25.0)),
            ],
            NotificationEngine::DEFAULT_COOLDOWN,
        );

        engine.observe(&reading("2026-08-01T10:00:00Z", 60, RESET));
        assert!(
            engine
                .observe(&reading("2026-08-01T10:00:30Z", 45, RESET))
                .is_empty()
        );
        assert_eq!(
            engine.observe(&reading("2026-08-01T10:01:00Z", 20, RESET)),
            vec![crossed(25.0, 20.0)]
        );
    }

    /// Landing exactly on the threshold is a crossing — the rule reads "at or
    /// below".
    #[test]
    fn landing_exactly_on_a_threshold_fires() {
        let mut engine = NotificationEngine::default();

        engine.observe(&reading("2026-08-01T10:00:00Z", 60, RESET));
        assert_eq!(
            engine.observe(&reading("2026-08-01T10:00:30Z", 50, RESET)),
            vec![crossed(50.0, 50.0)]
        );
    }

    #[test]
    fn a_snapshot_without_a_measurable_window_says_nothing() {
        let mut engine = NotificationEngine::default();
        let unmeasurable = UsageSnapshot::new(
            instant("2026-08-01T10:00:30Z"),
            Plan::Free,
            vec![UsageWindow::unbounded(WindowKind::Session, 40)],
            Source::new("mock"),
        );

        engine.observe(&reading("2026-08-01T10:00:00Z", 60, RESET));
        assert!(engine.observe(&unmeasurable).is_empty());
    }

    #[test]
    fn resetting_the_engine_makes_the_next_reading_a_starting_point() {
        let mut engine = NotificationEngine::default();

        engine.observe(&reading("2026-08-01T10:00:00Z", 60, RESET));
        engine.reset();

        assert!(
            engine
                .observe(&reading("2026-08-01T10:00:30Z", 45, RESET))
                .is_empty(),
            "there is nothing to have crossed from"
        );
    }

    #[test]
    fn the_default_rules_are_the_ones_from_the_spec() {
        let thresholds: Vec<_> = NotificationEngine::default()
            .rules()
            .into_iter()
            .map(|rule| rule.threshold_percent)
            .collect();

        assert_eq!(
            thresholds,
            vec![percent(50.0), percent(25.0), percent(10.0), percent(5.0)]
        );
    }
}
