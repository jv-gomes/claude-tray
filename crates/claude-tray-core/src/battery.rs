//! How full the battery icon should look.

use core::fmt;

use crate::Percent;

/// How many of the icon's segments are filled.
///
/// The icon is a battery with [`BatteryLevel::SEGMENTS`] segments, so a
/// continuous percentage has to be quantised before it can be drawn. Doing that
/// here rather than in the renderer means the icon cache is keyed on ten
/// possible fills instead of on every distinct percentage, and it is what allows
/// a redraw to be skipped when the number moves but the picture would not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BatteryLevel(u8);

impl BatteryLevel {
    /// How many segments the icon has.
    pub const SEGMENTS: u8 = 10;

    /// Nothing left — no segment lit.
    pub const EMPTY: Self = Self(0);

    /// Every segment lit.
    pub const FULL: Self = Self(Self::SEGMENTS);

    /// Quantises a remaining-usage figure onto the icon's segments.
    ///
    /// Rounds **down**, so the icon never claims more than there is: 99%
    /// remaining lights nine segments, and only a true 100% lights all ten.
    ///
    /// The one exception is at the bottom. Any figure above zero lights at least
    /// one segment, because an empty icon reads as "quota exhausted" and 4%
    /// remaining is not that. Only an exact zero draws empty.
    #[must_use]
    pub fn from_percent(percent: Percent) -> Self {
        let value = percent.value();
        if value <= 0.0 {
            return Self::EMPTY;
        }

        // `value` is in `0..=100`, so the quotient is in `0..=10` and the cast
        // cannot truncate or lose a sign.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let filled = (value / 10.0).floor() as u8;

        Self(filled.clamp(1, Self::SEGMENTS))
    }

    /// Returns how many segments are lit, from 0 to [`Self::SEGMENTS`].
    #[must_use]
    pub fn filled(self) -> u8 {
        self.0
    }

    /// Returns how many segments are dark.
    #[must_use]
    pub fn empty(self) -> u8 {
        Self::SEGMENTS - self.0
    }
}

impl fmt::Display for BatteryLevel {
    /// Renders the level the way the README draws it — `████░░░░░░` — which
    /// makes a failing test say what the icon would have looked like.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for _ in 0..self.filled() {
            f.write_str("█")?;
        }
        for _ in 0..self.empty() {
            f.write_str("░")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BatteryLevel;
    use crate::Percent;
    use proptest::prelude::proptest;

    fn level(value: f64) -> BatteryLevel {
        BatteryLevel::from_percent(Percent::new(value).unwrap())
    }

    #[test]
    fn quantises_to_ten_segments() {
        assert_eq!(level(100.0), BatteryLevel::FULL);
        assert_eq!(level(80.0).filled(), 8);
        assert_eq!(level(60.0).filled(), 6);
        assert_eq!(level(40.0).filled(), 4);
        assert_eq!(level(20.0).filled(), 2);
        assert_eq!(level(10.0).filled(), 1);
        assert_eq!(level(0.0), BatteryLevel::EMPTY);
    }

    /// Rounding down keeps the icon from over-promising: 99% is not full.
    #[test]
    fn rounds_down_so_only_a_true_full_looks_full() {
        assert_eq!(level(99.9).filled(), 9);
        assert_eq!(level(89.9).filled(), 8);
        assert_eq!(BatteryLevel::FULL.filled(), 10);
    }

    /// An empty icon means "exhausted". A few percent left must not look like
    /// that, however far down it rounds.
    #[test]
    fn only_an_exact_zero_draws_empty() {
        assert_eq!(level(0.1).filled(), 1);
        assert_eq!(level(4.0).filled(), 1);
        assert_eq!(level(9.9).filled(), 1);
        assert_eq!(level(0.0).filled(), 0);
    }

    #[test]
    fn filled_and_empty_segments_account_for_the_whole_icon() {
        for value in [0.0, 7.0, 33.3, 50.0, 99.0, 100.0] {
            let level = level(value);
            assert_eq!(level.filled() + level.empty(), BatteryLevel::SEGMENTS);
        }
    }

    #[test]
    fn renders_the_way_the_readme_draws_it() {
        assert_eq!(level(100.0).to_string(), "██████████");
        assert_eq!(level(80.0).to_string(), "████████░░");
        assert_eq!(level(10.0).to_string(), "█░░░░░░░░░");
        assert_eq!(level(0.0).to_string(), "░░░░░░░░░░");
    }

    proptest! {
        /// P2-29: more usage left never lights fewer segments.
        #[test]
        fn quantisation_is_monotonic(lower in 0.0f64..=100.0, higher in 0.0f64..=100.0) {
            let (lower, higher) = if lower <= higher { (lower, higher) } else { (higher, lower) };
            assert!(
                level(lower).filled() <= level(higher).filled(),
                "{lower} lit more segments than {higher}"
            );
        }

        /// Whatever the figure, the result is drawable.
        #[test]
        fn every_level_is_within_the_icon(value in 0.0f64..=100.0) {
            assert!(level(value).filled() <= BatteryLevel::SEGMENTS);
        }
    }
}
