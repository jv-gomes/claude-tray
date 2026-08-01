//! How much usage is left, expressed as the colour the icon should be.

use core::fmt;

use crate::Percent;

/// The colour band a remaining-usage figure falls into.
///
/// This is the whole of the icon's colour vocabulary. The renderer maps these
/// five cases onto actual pixels, and a theme may map them onto shades of grey
/// — but nothing downstream re-derives a colour from a percentage, so a change
/// to the bands happens in exactly one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ColorState {
    /// Plenty left.
    Green,
    /// Moderate usage.
    Yellow,
    /// Running low.
    Orange,
    /// Nearly exhausted.
    Red,
    /// No usable figure — not signed in, still loading, or the provider
    /// reported no window with a limit.
    Unknown,
}

impl ColorState {
    /// Maps a remaining-usage figure onto a colour band.
    ///
    /// Each band is inclusive of its threshold: at exactly 75% remaining the
    /// icon is green, not yellow. This never returns [`Self::Unknown`], which
    /// describes the absence of a figure rather than a band it falls into — see
    /// [`Self::from_remaining`].
    #[must_use]
    pub fn from_percent(percent: Percent, thresholds: Thresholds) -> Self {
        if percent >= thresholds.green() {
            Self::Green
        } else if percent >= thresholds.yellow() {
            Self::Yellow
        } else if percent >= thresholds.orange() {
            Self::Orange
        } else {
            Self::Red
        }
    }

    /// Maps an optional figure onto a band, with `None` becoming
    /// [`Self::Unknown`].
    #[must_use]
    pub fn from_remaining(remaining: Option<Percent>, thresholds: Thresholds) -> Self {
        remaining.map_or(Self::Unknown, |percent| {
            Self::from_percent(percent, thresholds)
        })
    }

    /// Returns the band's stable name, for logs and icon asset lookup.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Orange => "orange",
            Self::Red => "red",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for ColorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where the colour bands begin.
///
/// Injectable rather than hardcoded because these are a user preference in all
/// but name: someone on a plan they rarely exhaust wants to be warned later
/// than someone who lives at the limit. Phase 10 reads them from configuration;
/// until then [`Thresholds::default`] carries the values from the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds {
    green: Percent,
    yellow: Percent,
    orange: Percent,
}

impl Thresholds {
    /// Creates a threshold set.
    ///
    /// # Errors
    ///
    /// Returns [`ThresholdsError::NotDescending`] unless
    /// `green > yellow > orange`. Equal or inverted thresholds would make a
    /// band unreachable, which is a configuration mistake worth reporting
    /// rather than a preference worth honouring.
    pub fn new(green: Percent, yellow: Percent, orange: Percent) -> Result<Self, ThresholdsError> {
        if green > yellow && yellow > orange {
            Ok(Self {
                green,
                yellow,
                orange,
            })
        } else {
            Err(ThresholdsError::NotDescending {
                green,
                yellow,
                orange,
            })
        }
    }

    /// The figure at or above which usage is [`ColorState::Green`].
    #[must_use]
    pub fn green(self) -> Percent {
        self.green
    }

    /// The figure at or above which usage is [`ColorState::Yellow`].
    #[must_use]
    pub fn yellow(self) -> Percent {
        self.yellow
    }

    /// The figure at or above which usage is [`ColorState::Orange`]; below it,
    /// [`ColorState::Red`].
    #[must_use]
    pub fn orange(self) -> Percent {
        self.orange
    }
}

impl Default for Thresholds {
    /// The bands from the spec: green from 75, yellow from 40, orange from 15,
    /// red below that.
    fn default() -> Self {
        Self {
            green: Percent::from_literal(75.0),
            yellow: Percent::from_literal(40.0),
            orange: Percent::from_literal(15.0),
        }
    }
}

/// Why a threshold set was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ThresholdsError {
    /// The thresholds were not strictly descending, so at least one band could
    /// never be reached.
    #[error(
        "thresholds must be strictly descending, got green {green}, yellow {yellow}, orange {orange}"
    )]
    NotDescending {
        /// The green threshold as supplied.
        green: Percent,
        /// The yellow threshold as supplied.
        yellow: Percent,
        /// The orange threshold as supplied.
        orange: Percent,
    },
}

#[cfg(test)]
mod tests {
    use super::{ColorState, Thresholds, ThresholdsError};
    use crate::Percent;

    fn percent(value: f64) -> Percent {
        Percent::new(value).unwrap()
    }

    fn band(value: f64) -> ColorState {
        ColorState::from_percent(percent(value), Thresholds::default())
    }

    /// P2-25: the bands are inclusive of their thresholds, and the value just
    /// below each one falls into the next band down.
    #[test]
    fn bands_are_inclusive_of_their_thresholds() {
        assert_eq!(band(100.0), ColorState::Green);
        assert_eq!(band(75.0), ColorState::Green);
        assert_eq!(band(74.9), ColorState::Yellow);
        assert_eq!(band(40.0), ColorState::Yellow);
        assert_eq!(band(39.9), ColorState::Orange);
        assert_eq!(band(15.0), ColorState::Orange);
        assert_eq!(band(14.9), ColorState::Red);
        assert_eq!(band(0.0), ColorState::Red);
    }

    #[test]
    fn a_missing_figure_is_unknown_rather_than_red() {
        assert_eq!(
            ColorState::from_remaining(None, Thresholds::default()),
            ColorState::Unknown,
            "no data must not look like no quota"
        );
        assert_eq!(
            ColorState::from_remaining(Some(Percent::FULL), Thresholds::default()),
            ColorState::Green
        );
    }

    #[test]
    fn custom_thresholds_move_the_bands() {
        let cautious = Thresholds::new(percent(90.0), percent(60.0), percent(30.0)).unwrap();
        assert_eq!(
            ColorState::from_percent(percent(75.0), cautious),
            ColorState::Yellow,
            "the same figure that is green by default is yellow here"
        );
    }

    #[test]
    fn thresholds_must_be_strictly_descending() {
        assert_eq!(
            Thresholds::new(percent(40.0), percent(40.0), percent(15.0)).unwrap_err(),
            ThresholdsError::NotDescending {
                green: percent(40.0),
                yellow: percent(40.0),
                orange: percent(15.0),
            },
            "equal thresholds would make the yellow band unreachable"
        );
        assert!(Thresholds::new(percent(15.0), percent(40.0), percent(75.0)).is_err());
    }

    #[test]
    fn the_defaults_are_the_ones_from_the_spec() {
        let defaults = Thresholds::default();
        assert_eq!(defaults.green(), percent(75.0));
        assert_eq!(defaults.yellow(), percent(40.0));
        assert_eq!(defaults.orange(), percent(15.0));
    }
}
