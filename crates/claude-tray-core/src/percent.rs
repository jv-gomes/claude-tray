//! A percentage that is guaranteed to be a real number between 0 and 100.

use core::cmp::Ordering;
use core::fmt;

use serde::{Deserialize, Serialize};

/// A percentage in the closed range `0..=100`.
///
/// Every percentage in the application flows through this type, so the rest of
/// the code never has to consider what a negative, `NaN`, or 3000% usage figure
/// would mean. Providers hand back whatever their source reports; validation
/// happens once, here, at the boundary.
///
/// Because `NaN` cannot be constructed, this type has a total ordering and full
/// equality — which is what lets the most-constrained window be chosen with an
/// ordinary `min()`.
///
/// ```
/// use claude_tray_core::Percent;
///
/// let p = Percent::new(81.0)?;
/// assert_eq!(p.value(), 81.0);
/// assert!(Percent::new(101.0).is_err());
/// # Ok::<(), claude_tray_core::PercentError>(())
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Percent(f64);

impl Percent {
    /// Nothing left.
    pub const ZERO: Self = Self(0.0);

    /// Completely full.
    pub const FULL: Self = Self(100.0);

    /// Creates a percentage, rejecting anything outside `0..=100`.
    ///
    /// Use this for values the application computes itself, where an
    /// out-of-range result is a bug worth surfacing.
    ///
    /// # Errors
    ///
    /// Returns [`PercentError::NotANumber`] for `NaN` and
    /// [`PercentError::OutOfRange`] for anything below 0 or above 100,
    /// infinities included.
    pub fn new(value: f64) -> Result<Self, PercentError> {
        if value.is_nan() {
            return Err(PercentError::NotANumber);
        }
        if !(0.0..=100.0).contains(&value) {
            return Err(PercentError::OutOfRange(value));
        }
        Ok(Self(normalise(value)))
    }

    /// Creates a percentage, clamping out-of-range values to the nearest bound.
    ///
    /// Use this for values that come from outside the application, where a
    /// provider reporting 100.0000001% is noise rather than a fault.
    ///
    /// # Errors
    ///
    /// Returns [`PercentError::NotANumber`] for `NaN`, which has no nearest
    /// bound to clamp to.
    pub fn saturating(value: f64) -> Result<Self, PercentError> {
        if value.is_nan() {
            return Err(PercentError::NotANumber);
        }
        Ok(Self(normalise(value.clamp(0.0, 100.0))))
    }

    /// Returns the percentage as a plain number in `0.0..=100.0`.
    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }

    /// Builds a percentage from a literal that is in range by inspection.
    ///
    /// Exists only for `const` items — [`Percent::new`] returns a `Result`,
    /// which no `const` can unwrap without panicking, and panicking is
    /// forbidden in this crate. Crate-private on purpose: the invariant here is
    /// upheld by reading the source, which is only sound for values written out
    /// as literals, and each use is covered by a test that compares it against
    /// [`Percent::new`].
    pub(crate) const fn from_literal(value: f64) -> Self {
        Self(value)
    }
}

/// Maps `-0.0` onto `+0.0` so that equality and ordering agree.
///
/// Adding zero leaves every other finite value untouched, and `NaN` has already
/// been rejected by the time this runs.
fn normalise(value: f64) -> f64 {
    value + 0.0
}

impl PartialEq for Percent {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Percent {}

impl PartialOrd for Percent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Percent {
    fn cmp(&self, other: &Self) -> Ordering {
        // Total by construction: no `NaN` and no negative zero can exist here.
        self.0.total_cmp(&other.0)
    }
}

impl TryFrom<f64> for Percent {
    type Error = PercentError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Percent> for f64 {
    fn from(percent: Percent) -> Self {
        percent.0
    }
}

/// Why a percentage could not be constructed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PercentError {
    /// The value was `NaN`, which is neither in nor out of range.
    #[error("percentage is not a number")]
    NotANumber,

    /// The value was below 0 or above 100.
    #[error("percentage {0} is outside the range 0..=100")]
    OutOfRange(f64),
}

impl fmt::Display for Percent {
    /// Renders the percentage without a unit, rounded to the nearest whole
    /// number — `81`, not `81.4%`. The unit belongs to the display layer, which
    /// knows whether it is writing a tooltip or a log line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.0}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{Percent, PercentError};
    use proptest::prelude::{Just, Strategy, any, prop_oneof, proptest};

    #[test]
    fn accepts_the_bounds() {
        assert_eq!(Percent::new(0.0).unwrap(), Percent::ZERO);
        assert_eq!(Percent::new(100.0).unwrap(), Percent::FULL);
    }

    #[test]
    fn rejects_out_of_range() {
        assert_eq!(
            Percent::new(-0.1).unwrap_err(),
            PercentError::OutOfRange(-0.1)
        );
        assert_eq!(
            Percent::new(100.1).unwrap_err(),
            PercentError::OutOfRange(100.1)
        );
        assert_eq!(
            Percent::new(f64::INFINITY).unwrap_err(),
            PercentError::OutOfRange(f64::INFINITY)
        );
    }

    #[test]
    fn rejects_nan_in_both_constructors() {
        assert_eq!(
            Percent::new(f64::NAN).unwrap_err(),
            PercentError::NotANumber
        );
        assert_eq!(
            Percent::saturating(f64::NAN).unwrap_err(),
            PercentError::NotANumber
        );
    }

    #[test]
    fn saturating_clamps_instead_of_failing() {
        assert_eq!(Percent::saturating(-3.0).unwrap(), Percent::ZERO);
        assert_eq!(Percent::saturating(100.000_001).unwrap(), Percent::FULL);
        assert_eq!(Percent::saturating(f64::INFINITY).unwrap(), Percent::FULL);
        assert_eq!(
            Percent::saturating(42.0).unwrap(),
            Percent::new(42.0).unwrap(),
            "an in-range value passes through untouched"
        );
    }

    /// `-0.0 == 0.0` but `total_cmp` orders them apart, which would make `Eq`
    /// and `Ord` disagree. The constructor normalises the sign away.
    #[test]
    fn negative_zero_equals_zero() {
        let negative = Percent::new(-0.0).unwrap();
        assert_eq!(negative, Percent::ZERO);
        assert!(negative.value().is_sign_positive());
    }

    #[test]
    fn orders_by_value() {
        let mut values = [
            Percent::new(40.0).unwrap(),
            Percent::ZERO,
            Percent::FULL,
            Percent::new(15.0).unwrap(),
        ];
        values.sort_unstable();
        assert_eq!(
            values,
            [
                Percent::ZERO,
                Percent::new(15.0).unwrap(),
                Percent::new(40.0).unwrap(),
                Percent::FULL,
            ],
            "sorting a set of windows is how the most constrained one is found"
        );
    }

    #[test]
    fn round_trips_through_json() {
        let percent = Percent::new(81.5).unwrap();
        let json = serde_json::to_string(&percent).unwrap();
        assert_eq!(json, "81.5");
        assert_eq!(serde_json::from_str::<Percent>(&json).unwrap(), percent);
    }

    /// Deserialisation must not be a back door around the invariant.
    #[test]
    fn deserialisation_validates() {
        assert!(serde_json::from_str::<Percent>("101").is_err());
        assert!(serde_json::from_str::<Percent>("-1").is_err());
    }

    #[test]
    fn displays_rounded_and_unitless() {
        assert_eq!(Percent::new(81.4).unwrap().to_string(), "81");
        assert_eq!(Percent::new(81.6).unwrap().to_string(), "82");
    }

    /// Any `f64` at all: in range, out of range, subnormal, infinite, or `NaN`.
    fn any_f64() -> impl Strategy<Value = f64> {
        prop_oneof![
            5 => -200.0f64..200.0,
            1 => any::<f64>(),
            1 => prop_oneof![
                Just(f64::NAN),
                Just(f64::INFINITY),
                Just(f64::NEG_INFINITY),
                Just(-0.0f64),
                Just(0.0f64),
                Just(100.0f64),
            ],
        ]
    }

    proptest! {
        /// P2-28: construction accepts exactly the in-range reals.
        #[test]
        fn new_accepts_exactly_the_valid_range(value in any_f64()) {
            let valid = !value.is_nan() && (0.0..=100.0).contains(&value);
            assert_eq!(Percent::new(value).is_ok(), valid);
        }

        /// Whatever survives construction satisfies the invariant.
        #[test]
        fn constructed_values_are_always_in_range(value in any_f64()) {
            for percent in [Percent::new(value), Percent::saturating(value)]
                .into_iter()
                .flatten()
            {
                assert!((0.0..=100.0).contains(&percent.value()));
                assert!(!percent.value().is_nan());
            }
        }

        /// `saturating` fails only on `NaN`.
        #[test]
        fn saturating_only_rejects_nan(value in any_f64()) {
            assert_eq!(Percent::saturating(value).is_ok(), !value.is_nan());
        }
    }
}
