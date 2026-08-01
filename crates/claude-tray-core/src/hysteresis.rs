//! Keeping a value that sits on a boundary from flickering the icon.

use crate::Percent;

/// Suppresses changes that a figure hovering on a boundary would otherwise
/// cause.
///
/// A percentage drifting across 75.0 — 74.98, 75.01, 74.99 — would repaint the
/// icon green, yellow, green with every refresh. Nothing is wrong with any of
/// those readings; the problem is that a boundary is a step function and real
/// data is noisy.
///
/// The rule: a figure may move the output only once it has gone at least
/// `margin` points *past* the boundary it crossed. Until then the previous
/// output stands. Because the reference point is the reading that last changed
/// the output, a value that genuinely drifts still gets through — it simply has
/// to travel the margin first.
///
/// This wraps any quantiser, so the same instance protects a
/// [`ColorState`](crate::ColorState) or a
/// [`BatteryLevel`](crate::BatteryLevel):
///
/// ```
/// use claude_tray_core::{ColorState, Hysteresis, Percent, Thresholds};
///
/// let thresholds = Thresholds::default();
/// let mut colors = Hysteresis::default();
/// let band = |percent| ColorState::from_percent(percent, thresholds);
///
/// assert_eq!(colors.update(Percent::new(76.0)?, band), ColorState::Green);
/// // A dip just past the boundary is noise, not a change of state.
/// assert_eq!(colors.update(Percent::new(74.9)?, band), ColorState::Green);
/// // A real fall gets through.
/// assert_eq!(colors.update(Percent::new(70.0)?, band), ColorState::Yellow);
/// # Ok::<(), claude_tray_core::PercentError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteresis<T> {
    margin: Percent,
    committed: Option<Committed<T>>,
}

/// The reading that last changed the output, and what it produced.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Committed<T> {
    percent: Percent,
    output: T,
}

impl<T> Hysteresis<T> {
    /// How far a figure must travel past a boundary by default: two percentage
    /// points, a fifth of a battery segment.
    pub const DEFAULT_MARGIN: Percent = Percent::from_literal(2.0);

    /// Creates a filter with the given margin, in percentage points.
    ///
    /// A margin of zero disables the filtering and passes every reading
    /// straight through.
    #[must_use]
    pub fn new(margin: Percent) -> Self {
        Self {
            margin,
            committed: None,
        }
    }

    /// Returns the output currently being held, if any reading has been seen.
    #[must_use]
    pub fn current(&self) -> Option<&T> {
        self.committed.as_ref().map(|held| &held.output)
    }

    /// Forgets the held reading, so the next one is taken at face value.
    ///
    /// Call this when continuity has been broken — a quota reset, a switch to a
    /// different provider — where holding on to the old output would suppress a
    /// change that is real.
    pub fn reset(&mut self) {
        self.committed = None;
    }
}

impl<T: Clone + PartialEq> Hysteresis<T> {
    /// Feeds in a reading and returns the output to render.
    ///
    /// `quantise` must be a pure function of the percentage — it is called more
    /// than once per reading, at the figure itself and at a probe point a margin
    /// away.
    pub fn update<F>(&mut self, percent: Percent, quantise: F) -> T
    where
        F: Fn(Percent) -> T,
    {
        let candidate = quantise(percent);

        // Taken by value so the held reading can be compared against while
        // `self` is borrowed to commit a new one.
        let Some(held) = self.committed.clone() else {
            return self.commit(percent, candidate);
        };

        if candidate == held.output {
            // No visible change, so nothing to suppress. The reading still
            // becomes the new reference point: without this, a value creeping
            // across a band would be measured from wherever it entered it.
            return self.commit(percent, candidate);
        }

        // The output would change. Probe a margin back towards where we last
        // committed: if the reading still lands in the old output from there,
        // it has not travelled far enough past the boundary to be believed.
        let margin = self.margin.value();
        let probe = if percent > held.percent {
            percent.value() - margin
        } else {
            percent.value() + margin
        };
        let probe = Percent::saturating(probe).unwrap_or(percent);

        if quantise(probe) == held.output {
            held.output
        } else {
            self.commit(percent, candidate)
        }
    }

    /// Accepts a reading as the new reference point and returns its output.
    fn commit(&mut self, percent: Percent, output: T) -> T {
        self.committed = Some(Committed {
            percent,
            output: output.clone(),
        });
        output
    }
}

impl<T> Default for Hysteresis<T> {
    fn default() -> Self {
        Self::new(Self::DEFAULT_MARGIN)
    }
}

#[cfg(test)]
mod tests {
    use super::Hysteresis;
    use crate::{BatteryLevel, ColorState, Percent, Thresholds};

    fn percent(value: f64) -> Percent {
        Percent::new(value).unwrap()
    }

    fn band(percent: Percent) -> ColorState {
        ColorState::from_percent(percent, Thresholds::default())
    }

    #[test]
    fn the_first_reading_is_always_taken_at_face_value() {
        let mut filter = Hysteresis::default();
        assert_eq!(filter.update(percent(74.9), band), ColorState::Yellow);
        assert_eq!(filter.current(), Some(&ColorState::Yellow));
    }

    /// P2-26: a figure oscillating around 75 must not repaint the icon.
    #[test]
    fn an_oscillating_figure_holds_its_band() {
        let mut filter = Hysteresis::default();
        let oscillation = [76.0, 74.9, 75.2, 74.8, 75.1, 74.95, 75.3];

        let bands: Vec<_> = oscillation
            .into_iter()
            .map(|value| filter.update(percent(value), band))
            .collect();

        assert_eq!(
            bands,
            vec![ColorState::Green; oscillation.len()],
            "every reading is within the margin of 75, so the band never moves"
        );
    }

    /// The margin buys stability, not stasis: a fall that clears it gets
    /// through on the reading that clears it.
    #[test]
    fn a_real_fall_gets_through() {
        let mut filter = Hysteresis::default();
        assert_eq!(filter.update(percent(76.0), band), ColorState::Green);
        assert_eq!(filter.update(percent(74.5), band), ColorState::Green);
        assert_eq!(filter.update(percent(72.9), band), ColorState::Yellow);
    }

    /// A value creeping down in steps smaller than the margin must still change
    /// band eventually, or the icon would freeze on a slow drain.
    #[test]
    fn a_slow_drift_is_not_suppressed_forever() {
        let mut filter = Hysteresis::default();
        let mut value = 80.0;

        while value > 60.0 {
            filter.update(percent(value), band);
            value -= 0.5;
        }

        assert_eq!(
            filter.current(),
            Some(&ColorState::Yellow),
            "twenty points of drift must be believed however small the steps"
        );
    }

    #[test]
    fn rising_past_a_boundary_needs_the_margin_too() {
        let mut filter = Hysteresis::default();
        assert_eq!(filter.update(percent(74.0), band), ColorState::Yellow);
        assert_eq!(
            filter.update(percent(76.0), band),
            ColorState::Yellow,
            "just over the line is not enough to go green"
        );
        assert_eq!(filter.update(percent(77.5), band), ColorState::Green);
    }

    #[test]
    fn a_zero_margin_passes_everything_through() {
        let mut filter = Hysteresis::new(Percent::ZERO);
        assert_eq!(filter.update(percent(75.0), band), ColorState::Green);
        assert_eq!(filter.update(percent(74.9), band), ColorState::Yellow);
        assert_eq!(filter.update(percent(75.0), band), ColorState::Green);
    }

    #[test]
    fn resetting_takes_the_next_reading_at_face_value() {
        let mut filter = Hysteresis::default();
        assert_eq!(filter.update(percent(76.0), band), ColorState::Green);
        filter.reset();
        assert_eq!(filter.current(), None);
        assert_eq!(filter.update(percent(74.9), band), ColorState::Yellow);
    }

    /// The same filter protects the battery quantiser, whose boundaries sit
    /// every ten points rather than at the colour thresholds.
    #[test]
    fn it_wraps_the_battery_quantiser_too() {
        let mut filter = Hysteresis::default();
        assert_eq!(
            filter
                .update(percent(80.4), BatteryLevel::from_percent)
                .filled(),
            8
        );
        assert_eq!(
            filter
                .update(percent(79.6), BatteryLevel::from_percent)
                .filled(),
            8,
            "a dip just under 80 is not a segment"
        );
        assert_eq!(
            filter
                .update(percent(77.0), BatteryLevel::from_percent)
                .filled(),
            7
        );
    }

    /// Boundaries at the ends of the scale have nothing beyond them to probe;
    /// clamping the probe must not wedge the filter.
    #[test]
    fn the_ends_of_the_scale_still_move() {
        let mut filter = Hysteresis::default();
        assert_eq!(filter.update(percent(1.0), band), ColorState::Red);
        assert_eq!(filter.update(percent(100.0), band), ColorState::Green);
        assert_eq!(filter.update(percent(0.0), band), ColorState::Red);
    }
}
