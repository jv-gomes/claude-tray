//! Which colours the icon is drawn in.

use claude_tray_core::ColorState;

/// An opaque 8-bit colour.
///
/// Alpha is applied at draw time — for staleness, for the empty part of the
/// battery — rather than being carried here, so that two themes differing only
/// in opacity cannot exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Rgb {
    /// Creates a colour.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Creates a colour from `0xRRGGBB`.
    #[must_use]
    pub const fn hex(value: u32) -> Self {
        Self {
            r: ((value >> 16) & 0xFF) as u8,
            g: ((value >> 8) & 0xFF) as u8,
            b: (value & 0xFF) as u8,
        }
    }

    /// Relative luminance, per WCAG 2.x, for contrast checks.
    #[must_use]
    pub fn luminance(self) -> f32 {
        fn channel(value: u8) -> f32 {
            let value = f32::from(value) / 255.0;
            if value <= 0.039_28 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }

        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    /// Contrast ratio against another colour, from 1.0 to 21.0.
    #[must_use]
    pub fn contrast_against(self, other: Self) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        let (lighter, darker) = if a >= b { (a, b) } else { (b, a) };
        (lighter + 0.05) / (darker + 0.05)
    }
}

/// What the user asked the icon to look like.
///
/// This is a preference, and two of its variants cannot be drawn as they stand:
/// [`Self::Auto`] needs to know whether the panel is light or dark, and
/// [`Self::SystemAccent`] needs the accent colour itself. Both come from the
/// platform, which this crate deliberately does not talk to — call
/// [`Preference::resolve`] at the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Preference {
    /// Follow the system's light or dark appearance.
    #[default]
    Auto,
    /// Colours tuned for a light panel.
    Light,
    /// Colours tuned for a dark panel.
    Dark,
    /// One colour plus alpha, for panels that do not want colour at all.
    Monochrome,
    /// The system accent colour, with the fill level carrying the whole signal.
    SystemAccent,
    /// A palette that does not rely on the red–green axis.
    ColorBlindSafe,
}

impl Preference {
    /// Turns a preference into something drawable.
    ///
    /// `accent` is what the platform reports, and is ignored unless the
    /// preference asks for it; a preference for an accent colour the platform
    /// will not give falls back to following the appearance, because an icon
    /// that fails to draw is worse than one in the wrong colour.
    #[must_use]
    pub fn resolve(self, appearance: Appearance, accent: Option<Rgb>) -> Theme {
        match self {
            Self::Auto => appearance.theme(),
            Self::Light => Theme::Light,
            Self::Dark => Theme::Dark,
            Self::Monochrome => Theme::Monochrome(appearance),
            Self::ColorBlindSafe => Theme::ColorBlindSafe(appearance),
            Self::SystemAccent => accent.map_or_else(|| appearance.theme(), Theme::Accent),
        }
    }
}

/// Whether the surface the icon sits on is light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Appearance {
    /// A light panel; the icon is drawn in dark colours.
    Light,
    /// A dark panel; the icon is drawn in light colours.
    #[default]
    Dark,
}

impl Appearance {
    /// The theme that follows this appearance.
    #[must_use]
    pub fn theme(self) -> Theme {
        match self {
            Self::Light => Theme::Light,
            Self::Dark => Theme::Dark,
        }
    }
}

/// A theme that can actually be drawn.
///
/// Every variant is hashable and carries everything the renderer needs, so an
/// [`IconSpec`](crate::IconSpec) built from one is a complete cache key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Theme {
    /// For a light panel.
    Light,
    /// For a dark panel.
    Dark,
    /// A single colour plus alpha, chosen to contrast with the panel.
    Monochrome(Appearance),
    /// Black plus alpha, for a macOS template image the menu bar tints itself.
    ///
    /// Always black regardless of appearance — that is what macOS expects, and
    /// tinting it here would fight the system rather than cooperate with it.
    /// Phase 6 must mark the image as a template, or it will render as a black
    /// blob in dark mode.
    Template,
    /// One supplied colour, with the fill level carrying the whole signal.
    Accent(Rgb),
    /// A palette avoiding the red–green axis.
    ColorBlindSafe(Appearance),
}

impl Theme {
    /// The colours to draw a given band in.
    #[must_use]
    pub fn palette(self, color_state: ColorState) -> Palette {
        match self {
            Self::Light => Palette {
                shell: Rgb::hex(0x3C_41_51),
                fill: band(color_state, Appearance::Light),
                empty: Rgb::hex(0x3C_41_51),
            },
            Self::Dark => Palette {
                shell: Rgb::hex(0xE5_E9_F0),
                fill: band(color_state, Appearance::Dark),
                empty: Rgb::hex(0xE5_E9_F0),
            },
            Self::Monochrome(appearance) => {
                let ink = match appearance {
                    Appearance::Light => Rgb::hex(0x1C_1C_1C),
                    Appearance::Dark => Rgb::hex(0xF2_F2_F2),
                };
                Palette {
                    shell: ink,
                    fill: ink,
                    empty: ink,
                }
            }
            Self::Template => Palette {
                shell: Rgb::hex(0x00_00_00),
                fill: Rgb::hex(0x00_00_00),
                empty: Rgb::hex(0x00_00_00),
            },
            Self::Accent(accent) => Palette {
                shell: accent,
                fill: accent,
                empty: accent,
            },
            Self::ColorBlindSafe(appearance) => {
                let ink = match appearance {
                    Appearance::Light => Rgb::hex(0x3C_41_51),
                    Appearance::Dark => Rgb::hex(0xE5_E9_F0),
                };
                Palette {
                    shell: ink,
                    fill: safe_band(color_state),
                    empty: ink,
                }
            }
        }
    }

    /// Whether this theme carries meaning in hue at all.
    ///
    /// False for the themes where the fill level is the only signal, which is
    /// what makes them safe to use when hue cannot be relied on.
    #[must_use]
    pub fn is_colored(self) -> bool {
        matches!(self, Self::Light | Self::Dark | Self::ColorBlindSafe(_))
    }
}

/// The colours one icon is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Palette {
    /// The battery outline and its terminal nub.
    pub shell: Rgb,
    /// The filled portion.
    pub fill: Rgb,
    /// The unfilled portion, drawn at low alpha so the battery reads as a
    /// container rather than as a bar.
    pub empty: Rgb,
}

/// The default colour ramp, tuned per appearance.
///
/// Darker on a light panel and lighter on a dark one, because the same green
/// cannot carry contrast against both.
fn band(color_state: ColorState, appearance: Appearance) -> Rgb {
    match (color_state, appearance) {
        (ColorState::Green, Appearance::Light) => Rgb::hex(0x1B_7F_3B),
        (ColorState::Green, Appearance::Dark) => Rgb::hex(0x4A_D6_6D),
        (ColorState::Yellow, Appearance::Light) => Rgb::hex(0x9A_6B_00),
        (ColorState::Yellow, Appearance::Dark) => Rgb::hex(0xF2_C5_4E),
        (ColorState::Orange, Appearance::Light) => Rgb::hex(0xB2_53_00),
        (ColorState::Orange, Appearance::Dark) => Rgb::hex(0xFF_9A_3D),
        // Red on a dark panel is the worst case for contrast, so it is lifted
        // well above the usual pure red — see the contrast tests.
        (ColorState::Red, Appearance::Light) => Rgb::hex(0xC0_2A_1E),
        (ColorState::Red, Appearance::Dark) => Rgb::hex(0xFF_6B_5B),
        (ColorState::Unknown, Appearance::Light) => Rgb::hex(0x6B_71_80),
        (ColorState::Unknown, Appearance::Dark) => Rgb::hex(0x9A_A1_B2),
    }
}

/// A ramp that does not depend on telling red from green.
///
/// The Okabe–Ito qualitative palette, which is designed to stay distinguishable
/// under the common forms of colour vision deficiency. Severity still reads
/// left to right, and the fill level carries the signal regardless.
fn safe_band(color_state: ColorState) -> Rgb {
    match color_state {
        ColorState::Green => Rgb::hex(0x00_9E_73),
        ColorState::Yellow => Rgb::hex(0xF0_E4_42),
        ColorState::Orange => Rgb::hex(0xE6_9F_00),
        ColorState::Red => Rgb::hex(0xD5_5E_00),
        ColorState::Unknown => Rgb::hex(0x99_99_99),
    }
}

#[cfg(test)]
mod tests {
    use super::{Appearance, Preference, Rgb, Theme};
    use claude_tray_core::ColorState;

    const EVERY_BAND: [ColorState; 5] = [
        ColorState::Green,
        ColorState::Yellow,
        ColorState::Orange,
        ColorState::Red,
        ColorState::Unknown,
    ];

    /// What a light and a dark panel actually are, near enough for a contrast
    /// check: white, and the near-black most dark panels use.
    const LIGHT_PANEL: Rgb = Rgb::hex(0xFF_FF_FF);
    const DARK_PANEL: Rgb = Rgb::hex(0x1E_1E_1E);

    #[test]
    fn hex_unpacks_the_way_it_reads() {
        assert_eq!(Rgb::hex(0x11_22_33), Rgb::new(0x11, 0x22, 0x33));
        assert_eq!(Rgb::hex(0xFF_FF_FF), Rgb::new(255, 255, 255));
        assert_eq!(Rgb::hex(0x00_00_00), Rgb::new(0, 0, 0));
    }

    #[test]
    fn contrast_runs_from_one_to_twenty_one() {
        let white = Rgb::hex(0xFF_FF_FF);
        let black = Rgb::hex(0x00_00_00);

        assert!((white.contrast_against(black) - 21.0).abs() < 0.01);
        assert!((white.contrast_against(white) - 1.0).abs() < 0.01);
        assert!(
            (white.contrast_against(black) - black.contrast_against(white)).abs() < 0.001,
            "contrast is symmetric"
        );
    }

    /// P5-19. A tray icon is a non-text graphic, so WCAG asks for 3:1. The
    /// worst case is red on a dark panel, which is why the dark ramp does not
    /// use a pure red.
    #[test]
    fn every_band_holds_contrast_on_both_panels() {
        for (theme, panel, name) in [
            (Theme::Light, LIGHT_PANEL, "light"),
            (Theme::Dark, DARK_PANEL, "dark"),
        ] {
            for band in EVERY_BAND {
                let palette = theme.palette(band);
                let contrast = palette.fill.contrast_against(panel);

                assert!(
                    contrast >= 3.0,
                    "{band:?} fill on a {name} panel is {contrast:.2}:1, below the 3:1 minimum"
                );
                assert!(
                    palette.shell.contrast_against(panel) >= 3.0,
                    "{band:?} shell on a {name} panel is too close to it"
                );
            }
        }
    }

    #[test]
    fn monochrome_and_template_hold_contrast_too() {
        assert!(
            Theme::Monochrome(Appearance::Dark)
                .palette(ColorState::Red)
                .fill
                .contrast_against(DARK_PANEL)
                >= 3.0
        );
        assert!(
            Theme::Monochrome(Appearance::Light)
                .palette(ColorState::Red)
                .fill
                .contrast_against(LIGHT_PANEL)
                >= 3.0
        );
    }

    /// Back to sRGB, gamma included.
    ///
    /// Perceived difference lives in the encoded space; comparing linear values
    /// would understate how far apart two mid-tones look.
    fn encode(value: f32) -> f32 {
        let value = value.clamp(0.0, 1.0);
        if value <= 0.003_130_8 {
            255.0 * 12.92 * value
        } else {
            255.0 * (1.055 * value.powf(1.0 / 2.4) - 0.055)
        }
    }

    /// Simulates deuteranopia, the common form of red–green colour blindness,
    /// by the Viénot–Brettel–Mollon method: convert to LMS, discard the M cone's
    /// own response, and convert back.
    fn as_deuteranope(colour: Rgb) -> (f32, f32, f32) {
        fn linear(value: u8) -> f32 {
            let value = f32::from(value) / 255.0;
            if value <= 0.039_28 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }

        let (r, g, b) = (linear(colour.r), linear(colour.g), linear(colour.b));

        let long = 17.8824 * r + 43.5161 * g + 4.119_35 * b;
        let short = 0.029_956_6 * r + 0.184_309 * g + 1.467_09 * b;
        // The deuteranope's medium cone response is not independent: it is a
        // combination of the other two.
        let medium = 0.494_207 * long + 1.248_27 * short;

        (
            encode(0.080_944 * long - 0.130_504 * medium + 0.116_721 * short),
            encode(-0.010_249 * long + 0.054_019 * medium - 0.113_615 * short),
            encode(-0.000_365 * long - 0.004_122 * medium + 0.693_511 * short),
        )
    }

    /// How far apart two colours look to someone with deuteranopia.
    fn separation(a: Rgb, b: Rgb) -> f32 {
        let (ar, ag, ab) = as_deuteranope(a);
        let (br, bg, bb) = as_deuteranope(b);
        ((ar - br).powi(2) + (ag - bg).powi(2) + (ab - bb).powi(2)).sqrt()
    }

    /// P5-20. Luminance contrast is the wrong test here: the Okabe–Ito bluish
    /// green and vermillion sit at almost the same brightness, and they are
    /// nonetheless easy to tell apart because they differ along the blue–yellow
    /// axis, which red–green deficiency leaves intact. So simulate the
    /// deficiency and measure what is actually left.
    #[test]
    fn the_safe_palette_survives_red_green_deficiency() {
        let safe = Theme::ColorBlindSafe(Appearance::Dark);
        let safe_ends = separation(
            safe.palette(ColorState::Green).fill,
            safe.palette(ColorState::Red).fill,
        );
        let default_ends = separation(
            Theme::Dark.palette(ColorState::Green).fill,
            Theme::Dark.palette(ColorState::Red).fill,
        );

        assert!(
            safe_ends > default_ends * 2.0,
            "the safe palette separates its ends by {safe_ends:.3} against {default_ends:.3} \
             for the default one, which is not enough of an improvement to be worth offering"
        );

        // Adjacent bands have to be distinguishable too, not just the extremes.
        for (lower, upper) in [
            (ColorState::Red, ColorState::Orange),
            (ColorState::Orange, ColorState::Yellow),
            (ColorState::Yellow, ColorState::Green),
        ] {
            let apart = separation(safe.palette(lower).fill, safe.palette(upper).fill);
            assert!(
                apart > default_ends,
                "{lower:?} and {upper:?} are only {apart:.3} apart under deuteranopia"
            );
        }
    }

    /// Being distinguishable is not enough on its own — the bands still have to
    /// be visible against the panel they sit on.
    #[test]
    fn the_safe_palette_still_holds_contrast() {
        let theme = Theme::ColorBlindSafe(Appearance::Dark);

        for band in EVERY_BAND {
            let contrast = theme.palette(band).fill.contrast_against(DARK_PANEL);
            assert!(
                contrast >= 3.0,
                "{band:?} is {contrast:.2}:1 on a dark panel"
            );
        }
    }

    /// Colour is never the only carrier: the fill level says the same thing,
    /// which is what lets these themes drop hue entirely.
    #[test]
    fn the_uncoloured_themes_say_so() {
        assert!(!Theme::Monochrome(Appearance::Dark).is_colored());
        assert!(!Theme::Template.is_colored());
        assert!(!Theme::Accent(Rgb::hex(0x00_7A_CC)).is_colored());
        assert!(Theme::Dark.is_colored());
        assert!(Theme::ColorBlindSafe(Appearance::Dark).is_colored());
    }

    #[test]
    fn monochrome_draws_one_colour_for_every_band() {
        let theme = Theme::Monochrome(Appearance::Dark);
        let first = theme.palette(ColorState::Green);

        for band in EVERY_BAND {
            assert_eq!(theme.palette(band), first, "{band:?} differs");
        }
    }

    #[test]
    fn auto_follows_the_panel() {
        assert_eq!(
            Preference::Auto.resolve(Appearance::Light, None),
            Theme::Light
        );
        assert_eq!(
            Preference::Auto.resolve(Appearance::Dark, None),
            Theme::Dark
        );
    }

    #[test]
    fn an_explicit_preference_ignores_the_panel() {
        assert_eq!(
            Preference::Light.resolve(Appearance::Dark, None),
            Theme::Light
        );
        assert_eq!(
            Preference::Dark.resolve(Appearance::Light, None),
            Theme::Dark
        );
    }

    #[test]
    fn the_accent_preference_uses_the_colour_the_platform_gave() {
        let accent = Rgb::hex(0x00_7A_CC);
        assert_eq!(
            Preference::SystemAccent.resolve(Appearance::Dark, Some(accent)),
            Theme::Accent(accent)
        );
    }

    /// An icon that fails to draw is worse than one in the wrong colour.
    #[test]
    fn an_accent_the_platform_will_not_give_falls_back() {
        assert_eq!(
            Preference::SystemAccent.resolve(Appearance::Dark, None),
            Theme::Dark
        );
        assert_eq!(
            Preference::SystemAccent.resolve(Appearance::Light, None),
            Theme::Light
        );
    }

    #[test]
    fn the_accent_is_ignored_unless_it_was_asked_for() {
        let accent = Some(Rgb::hex(0x00_7A_CC));
        assert_eq!(
            Preference::Dark.resolve(Appearance::Light, accent),
            Theme::Dark
        );
        assert_eq!(
            Preference::Monochrome.resolve(Appearance::Light, accent),
            Theme::Monochrome(Appearance::Light)
        );
    }
}
