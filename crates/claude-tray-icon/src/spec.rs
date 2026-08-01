//! What to draw, as a value that can be compared and cached.

use claude_tray_core::{BatteryLevel, ColorState, Percent, Thresholds, UsageState};

use crate::Theme;

/// One icon, described completely.
///
/// Everything the renderer needs and nothing it does not, so two specs that are
/// equal must produce identical pixels — which is what makes the render cache
/// sound.
///
/// Deliberately free of floating point, so it can be hashed. Sizes are whole
/// physical pixels and there is no scale factor here: the caller multiplies the
/// nominal size by the device pixel ratio and asks for the result. See
/// [`IconSize::for_scale`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IconSpec {
    /// How full the battery is drawn.
    pub level: BatteryLevel,
    /// Which colour band the fill takes.
    pub color_state: ColorState,
    /// The resolved theme — light, dark, monochrome, or an accent colour.
    pub theme: Theme,
    /// The size to render, in physical pixels.
    pub size: IconSize,
    /// Whether the reading behind this icon is too old to be trusted, which
    /// draws the whole icon faded.
    pub stale: bool,
    /// A badge drawn over the battery, for states that are not a reading.
    pub overlay: Overlay,
}

impl IconSpec {
    /// Describes the icon for a domain state.
    ///
    /// This is the one place the application's state becomes a picture, so that
    /// "what does an error look like" is answered once rather than at every call
    /// site.
    #[must_use]
    pub fn for_state(
        state: &UsageState,
        thresholds: Thresholds,
        theme: Theme,
        size: IconSize,
    ) -> Self {
        let remaining = state
            .snapshot()
            .and_then(claude_tray_core::UsageSnapshot::remaining_percent);

        Self {
            level: remaining.map_or(BatteryLevel::EMPTY, BatteryLevel::from_percent),
            color_state: ColorState::from_remaining(remaining, thresholds),
            theme,
            size,
            // A stale reading is still a reading: it keeps its level and colour
            // and is drawn faded, because an old number beats no number.
            stale: matches!(state, UsageState::Stale { .. }),
            overlay: match state {
                UsageState::Loading => Overlay::Loading,
                UsageState::Error(_) => Overlay::Error,
                UsageState::Unknown | UsageState::Ok(_) | UsageState::Stale { .. } => Overlay::None,
            },
        }
    }

    /// Describes an ordinary reading.
    #[must_use]
    pub fn from_percent(
        remaining: Percent,
        thresholds: Thresholds,
        theme: Theme,
        size: IconSize,
    ) -> Self {
        Self {
            level: BatteryLevel::from_percent(remaining),
            color_state: ColorState::from_percent(remaining, thresholds),
            theme,
            size,
            stale: false,
            overlay: Overlay::None,
        }
    }

    /// Whether the battery outline should be dashed rather than solid.
    ///
    /// Reserved for having no figure at all. A dashed shell reads as "nothing to
    /// report" without resembling an empty battery, which would mean the
    /// opposite of what it is.
    #[must_use]
    pub fn is_outlined(self) -> bool {
        self.color_state == ColorState::Unknown && self.overlay != Overlay::Loading
    }

    /// Returns the same icon at a different size, for a scale-factor change.
    #[must_use]
    pub fn at_size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }
}

/// A badge drawn over the battery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Overlay {
    /// Nothing over the battery.
    #[default]
    None,
    /// The first fetch is in flight and there is nothing to show yet.
    Loading,
    /// Usage could not be read.
    Error,
}

/// An icon size in physical pixels.
///
/// A newtype rather than a bare `u32` so that a scale factor cannot be passed
/// where a pixel count belongs, and so the bounds are checked once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IconSize(u32);

impl IconSize {
    /// The smallest size worth drawing this shape at. Below it the battery
    /// outline and its fill cannot both survive rounding.
    pub const MIN: u32 = 8;

    /// An upper bound, well past any tray but low enough that a bad scale
    /// factor cannot ask for a gigabyte of pixels.
    pub const MAX: u32 = 512;

    /// The nominal sizes platforms actually ask for, before scaling.
    ///
    /// 16 on the Windows notification area, 22 on most Linux panels, 24 on
    /// GNOME, 32 and up once a scale factor is applied.
    pub const NOMINAL: [u32; 6] = [16, 22, 24, 32, 48, 64];

    /// 16 px — the Windows notification area, and Linux panels at their
    /// smallest.
    pub const SMALL: Self = Self(16);

    /// 22 px — a typical Linux panel.
    pub const MEDIUM: Self = Self(22);

    /// 32 px — a 16 px icon on a 2× display.
    pub const LARGE: Self = Self(32);

    /// Creates a size.
    ///
    /// # Errors
    ///
    /// Returns [`IconSizeError`] outside [`Self::MIN`]`..=`[`Self::MAX`].
    pub fn new(pixels: u32) -> Result<Self, IconSizeError> {
        if (Self::MIN..=Self::MAX).contains(&pixels) {
            Ok(Self(pixels))
        } else {
            Err(IconSizeError(pixels))
        }
    }

    /// Returns the physical size to render a nominal size at, on a display with
    /// the given scale factor.
    ///
    /// Rounds to a whole pixel and clamps into range, because a scale factor
    /// comes from the platform and this must not fail at paint time. Rendering
    /// at the device pixel ratio is the entire point: a 16 px icon upscaled to
    /// 32 is exactly the blur this avoids.
    #[must_use]
    pub fn for_scale(nominal: u32, scale: f64) -> Self {
        let scaled = if scale.is_finite() && scale > 0.0 {
            (f64::from(nominal) * scale).round()
        } else {
            f64::from(nominal)
        };

        // The value is clamped into `MIN..=MAX` before the cast, so it cannot
        // truncate or lose a sign.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let pixels = scaled.clamp(f64::from(Self::MIN), f64::from(Self::MAX)) as u32;

        Self(pixels)
    }

    /// Returns the size in physical pixels.
    #[must_use]
    pub fn pixels(self) -> u32 {
        self.0
    }

    /// Returns the size as a float, for geometry.
    #[must_use]
    pub fn as_f32(self) -> f32 {
        // Every value in range is exactly representable.
        #[allow(clippy::cast_precision_loss)]
        {
            self.0 as f32
        }
    }
}

impl Default for IconSize {
    fn default() -> Self {
        Self::MEDIUM
    }
}

/// A size outside the drawable range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("icon size {0}px is outside {min}..={max}", min = IconSize::MIN, max = IconSize::MAX)]
pub struct IconSizeError(u32);

#[cfg(test)]
mod tests {
    use super::{IconSize, IconSpec, Overlay};
    use crate::Theme;
    use chrono::{DateTime, Utc};
    use claude_tray_core::{
        BatteryLevel, ColorState, ErrorKind, Percent, Plan, Source, Thresholds, UsageSnapshot,
        UsageState, UsageWindow, WindowKind,
    };

    fn instant(text: &str) -> DateTime<Utc> {
        text.parse().unwrap()
    }

    fn snapshot(remaining: u64) -> UsageSnapshot {
        UsageSnapshot::new(
            instant("2026-08-01T10:00:00Z"),
            vec![UsageWindow::new(
                WindowKind::Session,
                100 - remaining,
                100,
                instant("2026-08-01T18:00:00Z"),
            )],
            Source::new("mock"),
        )
        .with_plan(Plan::Pro)
    }

    fn spec(state: &UsageState) -> IconSpec {
        IconSpec::for_state(state, Thresholds::default(), Theme::Dark, IconSize::MEDIUM)
    }

    #[test]
    fn a_reading_becomes_a_level_and_a_colour() {
        let icon = spec(&UsageState::Ok(snapshot(80)));

        assert_eq!(icon.level.filled(), 8);
        assert_eq!(icon.color_state, ColorState::Green);
        assert_eq!(icon.overlay, Overlay::None);
        assert!(!icon.stale);
        assert!(!icon.is_outlined());
    }

    /// A stale reading keeps its number and is drawn faded. Dropping to an
    /// unknown icon would throw away information the user can still use.
    #[test]
    fn a_stale_reading_keeps_its_level_and_colour() {
        let stale = UsageState::Ok(snapshot(30)).into_stale(instant("2026-08-01T11:00:00Z"));
        let icon = spec(&stale);

        assert!(icon.stale);
        assert_eq!(icon.level.filled(), 3);
        assert_eq!(icon.color_state, ColorState::Orange);
        assert_eq!(icon.overlay, Overlay::None);
    }

    /// No data must not look like no quota — an empty red battery would say
    /// exactly the wrong thing.
    #[test]
    fn having_no_figure_is_drawn_as_an_outline_not_an_empty_battery() {
        let icon = spec(&UsageState::Unknown);

        assert_eq!(icon.color_state, ColorState::Unknown);
        assert!(icon.is_outlined());
        assert_ne!(icon.color_state, ColorState::Red);
    }

    #[test]
    fn loading_and_error_carry_their_own_badge() {
        assert_eq!(spec(&UsageState::Loading).overlay, Overlay::Loading);
        assert_eq!(
            spec(&UsageState::Error(ErrorKind::NotAuthenticated)).overlay,
            Overlay::Error
        );
    }

    /// The loading badge has its own shape, so the shell underneath it stays
    /// solid rather than becoming a second unknown-looking outline.
    #[test]
    fn loading_is_not_drawn_as_an_outline() {
        assert!(!spec(&UsageState::Loading).is_outlined());
    }

    /// A provider that reports usage without a limit gives a snapshot with no
    /// percentage. That is the unknown icon, not an empty one.
    #[test]
    fn a_snapshot_without_a_percentage_is_unknown() {
        let no_limit = UsageSnapshot::new(
            instant("2026-08-01T10:00:00Z"),
            vec![UsageWindow::unbounded(WindowKind::Session, 900)],
            Source::new("local-file"),
        );
        let icon = spec(&UsageState::Ok(no_limit));

        assert_eq!(icon.color_state, ColorState::Unknown);
        assert_eq!(icon.level, BatteryLevel::EMPTY);
        assert!(icon.is_outlined());
    }

    #[test]
    fn from_percent_describes_an_ordinary_reading() {
        let icon = IconSpec::from_percent(
            Percent::new(42.0).unwrap(),
            Thresholds::default(),
            Theme::Light,
            IconSize::SMALL,
        );

        assert_eq!(icon.level.filled(), 4);
        assert_eq!(icon.color_state, ColorState::Yellow);
        assert_eq!(icon.size, IconSize::SMALL);
    }

    #[test]
    fn equal_specs_hash_alike_so_the_cache_can_key_on_them() {
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        assert!(seen.insert(spec(&UsageState::Ok(snapshot(80)))));
        assert!(
            !seen.insert(spec(&UsageState::Ok(snapshot(80)))),
            "the same state must describe the same icon"
        );
        assert!(seen.insert(spec(&UsageState::Ok(snapshot(20)))));
    }

    #[test]
    fn sizes_outside_the_drawable_range_are_rejected() {
        assert!(IconSize::new(IconSize::MIN).is_ok());
        assert!(IconSize::new(IconSize::MAX).is_ok());
        assert!(IconSize::new(IconSize::MIN - 1).is_err());
        assert!(IconSize::new(IconSize::MAX + 1).is_err());
        assert!(IconSize::new(0).is_err());
    }

    /// Rendering at the device pixel ratio is the whole HiDPI story: a 16 px
    /// icon on a 2× display is a 32 px render, never a 16 px one scaled up.
    #[test]
    fn a_scale_factor_becomes_physical_pixels() {
        assert_eq!(IconSize::for_scale(16, 1.0).pixels(), 16);
        assert_eq!(IconSize::for_scale(16, 2.0).pixels(), 32);
        assert_eq!(IconSize::for_scale(22, 1.5).pixels(), 33);
        assert_eq!(IconSize::for_scale(24, 1.25).pixels(), 30);
    }

    /// A scale factor comes from the platform, and painting must not fail
    /// because it arrived as nonsense.
    #[test]
    fn a_nonsense_scale_factor_still_produces_a_drawable_size() {
        assert_eq!(IconSize::for_scale(16, 0.0).pixels(), 16);
        assert_eq!(IconSize::for_scale(16, -2.0).pixels(), 16);
        assert_eq!(IconSize::for_scale(16, f64::NAN).pixels(), 16);
        assert_eq!(IconSize::for_scale(16, 1e9).pixels(), IconSize::MAX);
        assert_eq!(IconSize::for_scale(16, 0.01).pixels(), IconSize::MIN);
    }

    #[test]
    fn every_nominal_size_is_drawable() {
        for nominal in IconSize::NOMINAL {
            assert!(IconSize::new(nominal).is_ok(), "{nominal}");
        }
    }
}
