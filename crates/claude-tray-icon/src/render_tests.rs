//! Tests for the drawing itself.
//!
//! Sizes here are small literals under the crate's own control, so the casts a
//! measurement needs cannot lose anything.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

//!
//! Split from `render.rs` because the geometric checks need helpers of their
//! own, and because a rasteriser is easier to reason about when the assertions
//! about its output sit together.

use claude_tray_core::{BatteryLevel, ColorState, Percent, Thresholds};

use crate::{Appearance, Icon, IconRenderer, IconSize, IconSpec, Overlay, Rgb, Theme};

fn percent(value: f64) -> Percent {
    Percent::new(value).unwrap()
}

fn spec(remaining: f64, size: u32) -> IconSpec {
    IconSpec::from_percent(
        percent(remaining),
        Thresholds::default(),
        Theme::Dark,
        IconSize::new(size).unwrap(),
    )
}

fn render(spec: IconSpec) -> std::sync::Arc<Icon> {
    IconRenderer::new().render(spec).unwrap()
}

/// How wide the fill runs along the icon's middle row, in pixels.
///
/// Measured by looking for the fill colour rather than for "any pixel", so the
/// outline and the dim empty track do not count.
fn fill_width(icon: &Icon, expected: Rgb) -> u32 {
    let middle = icon.height() / 2;
    (0..icon.width())
        .filter(|x| {
            icon.pixel(*x, middle).is_some_and(|(r, g, b, a)| {
                a > 200
                    && r.abs_diff(expected.r) < 24
                    && g.abs_diff(expected.g) < 24
                    && b.abs_diff(expected.b) < 24
            })
        })
        .count()
        .try_into()
        .unwrap_or(0)
}

/// How many pixels are painted at all.
fn covered(icon: &Icon) -> usize {
    icon.rgba()
        .chunks_exact(4)
        .filter(|pixel| pixel.get(3).is_some_and(|alpha| *alpha > 0))
        .count()
}

#[test]
fn an_icon_is_square_and_the_size_that_was_asked_for() {
    for size in IconSize::NOMINAL {
        let icon = render(spec(70.0, size));

        assert_eq!(icon.width(), size);
        assert_eq!(icon.height(), size);
        assert_eq!(
            u32::try_from(icon.rgba().len()).unwrap_or(u32::MAX),
            size * size * 4,
            "four bytes per pixel"
        );
    }
}

#[test]
fn something_is_actually_drawn_at_every_size() {
    for size in IconSize::NOMINAL {
        let icon = render(spec(70.0, size));
        let covered = covered(&icon);

        assert!(covered > 0, "{size}px drew nothing");
        assert!(
            covered < (size as usize) * (size as usize),
            "{size}px filled the whole canvas — the icon should have transparent margins"
        );
    }
}

/// The fill has to be monotonic in the level, or the icon would say something
/// other than what the number says.
///
/// Starts at one step, not zero: at exactly nothing left the shell itself takes
/// the alarm colour, which a probe looking for that colour cannot tell from a
/// fill. That state has its own test.
#[test]
fn more_remaining_never_draws_less_fill() {
    let green = Theme::Dark.palette(ColorState::Green).fill;
    let mut previous = 0;

    for step in 1..=10 {
        let icon = render(spec(f64::from(step) * 10.0, 64));
        let band = ColorState::from_percent(percent(f64::from(step) * 10.0), Thresholds::default());
        let width = fill_width(&icon, Theme::Dark.palette(band).fill);

        if step > 1 {
            assert!(
                width >= previous,
                "{}% drew {width}px of fill against {previous}px for the step below",
                step * 10
            );
        }
        previous = width;
    }

    assert!(fill_width(&render(spec(100.0, 64)), green) > 30);
}

/// The bug this caught: a padding derived from the stroke width jumped when the
/// stroke rounded up from 1 px to 2 px, so 24 px drew a visibly smaller fill
/// than 22 px for the same percentage.
#[test]
fn the_same_percentage_draws_the_same_proportion_at_every_size() {
    let fill = Theme::Dark.palette(ColorState::Yellow).fill;

    let proportions: Vec<f64> = IconSize::NOMINAL
        .iter()
        .map(|size| {
            let icon = render(spec(70.0, *size));
            f64::from(fill_width(&icon, fill)) / f64::from(*size)
        })
        .collect();

    let smallest = proportions.iter().copied().fold(f64::MAX, f64::min);
    let largest = proportions.iter().copied().fold(0.0_f64, f64::max);

    assert!(
        largest - smallest < 0.08,
        "the fill runs from {smallest:.3} to {largest:.3} of the icon width across sizes: \
         {proportions:?}"
    );
}

/// Empty must not look like "no reading", and it must not look like an ordinary
/// battery either — it is the most urgent state there is.
#[test]
fn nothing_left_is_drawn_in_the_alarm_colour() {
    let icon = render(spec(0.0, 64));
    let red = Theme::Dark.palette(ColorState::Red).fill;

    let alarming = icon
        .rgba()
        .chunks_exact(4)
        .filter(|pixel| {
            let (Some(r), Some(g), Some(b), Some(a)) =
                (pixel.first(), pixel.get(1), pixel.get(2), pixel.get(3))
            else {
                return false;
            };
            *a > 200 && r.abs_diff(red.r) < 24 && g.abs_diff(red.g) < 24 && b.abs_diff(red.b) < 24
        })
        .count();

    assert!(
        alarming > 0,
        "an exhausted quota drew nothing in the alarm colour, so it looks like an ordinary \
         empty battery"
    );
}

#[test]
fn an_unknown_state_draws_less_than_a_reading() {
    let reading = render(spec(70.0, 64));
    let unknown = render(IconSpec {
        color_state: ColorState::Unknown,
        level: BatteryLevel::EMPTY,
        ..spec(70.0, 64)
    });

    assert!(
        covered(&unknown) < covered(&reading),
        "a dashed outline with no fill should cover less than a filled battery"
    );
}

#[test]
fn a_stale_reading_is_drawn_faded() {
    let fresh = render(spec(70.0, 64));
    let stale = render(IconSpec {
        stale: true,
        ..spec(70.0, 64)
    });

    let alpha_sum = |icon: &Icon| -> u64 {
        icon.rgba()
            .chunks_exact(4)
            .filter_map(|pixel| pixel.get(3))
            .map(|alpha| u64::from(*alpha))
            .sum()
    };

    assert!(
        alpha_sum(&stale) < alpha_sum(&fresh) / 2,
        "a stale icon should be markedly fainter than a current one"
    );
    assert!(alpha_sum(&stale) > 0, "and still visible");
}

#[test]
fn the_error_badge_is_drawn_over_the_reading() {
    let plain = render(spec(70.0, 64));
    let errored = render(IconSpec {
        overlay: Overlay::Error,
        ..spec(70.0, 64)
    });

    assert_ne!(
        plain.rgba(),
        errored.rgba(),
        "the badge must change the pixels"
    );
    assert!(
        covered(&errored) > covered(&plain),
        "the badge adds to the icon rather than replacing it"
    );

    // The badge is a disc towards the bottom-right; probe its centre rather
    // than the very corner, which is outside it.
    let centre = errored.width() * 72 / 100;
    assert!(
        errored
            .pixel(centre, centre)
            .is_some_and(|(r, _, _, alpha)| alpha > 200 && r > 150),
        "nothing warning-coloured was drawn where the badge belongs"
    );
}

#[test]
fn loading_draws_something_other_than_a_fill() {
    let loading = render(IconSpec {
        overlay: Overlay::Loading,
        ..spec(70.0, 64)
    });
    let plain = render(spec(70.0, 64));

    assert_ne!(loading.rgba(), plain.rgba());
    assert!(covered(&loading) > 0);
    assert!(
        covered(&loading) < covered(&plain),
        "three dots should be lighter than a 70% fill"
    );
}

/// A template image is what macOS tints for itself: black plus alpha, nothing
/// else. Any colour in it would fight the menu bar rather than cooperate.
#[test]
fn the_template_theme_is_black_and_alpha_only() {
    let icon = render(IconSpec {
        theme: Theme::Template,
        ..spec(70.0, 64)
    });

    for pixel in icon.rgba().chunks_exact(4) {
        let (Some(r), Some(g), Some(b), Some(a)) =
            (pixel.first(), pixel.get(1), pixel.get(2), pixel.get(3))
        else {
            continue;
        };

        if *a == 0 {
            continue;
        }
        assert_eq!(
            (*r, *g, *b),
            (0, 0, 0),
            "a template image must carry no colour"
        );
    }
}

#[test]
fn a_theme_change_changes_the_pixels() {
    let base = spec(70.0, 32);
    let dark = render(base);

    for theme in [
        Theme::Light,
        Theme::Monochrome(Appearance::Dark),
        Theme::Template,
        Theme::Accent(Rgb::hex(0x00_7A_CC)),
        Theme::ColorBlindSafe(Appearance::Dark),
    ] {
        let other = render(IconSpec { theme, ..base });
        assert_ne!(dark.rgba(), other.rgba(), "{theme:?} rendered identically");
    }
}

/// `tiny-skia` composites premultiplied; every tray backend wants straight
/// alpha. Getting this wrong looks almost right — solid areas are unaffected —
/// and shows as dark fringing on every anti-aliased edge.
#[test]
fn the_buffer_is_straight_alpha_not_premultiplied() {
    let icon = render(spec(100.0, 64));
    // The shell, not the fill: the fill's edges composite over the dim track
    // behind it and come out opaque, whereas the outline is anti-aliased
    // against nothing at all, which is where partial alpha actually appears.
    let shell = Theme::Dark.palette(ColorState::Green).shell;

    // Under premultiplication every partially transparent pixel is scaled
    // towards black, so none of them could still carry the full fill colour.
    let bright_edge = icon.rgba().chunks_exact(4).any(|pixel| {
        let (Some(r), Some(g), Some(b), Some(a)) =
            (pixel.first(), pixel.get(1), pixel.get(2), pixel.get(3))
        else {
            return false;
        };

        (40..200).contains(a)
            && r.abs_diff(shell.r) < 16
            && g.abs_diff(shell.g) < 16
            && b.abs_diff(shell.b) < 16
    });

    assert!(
        bright_edge,
        "no anti-aliased pixel carries the full fill colour, which is what \
         premultiplied output looks like"
    );
}

#[test]
fn a_png_round_trips_back_to_the_same_pixels() {
    let icon = render(spec(70.0, 32));
    let png = icon.to_png().unwrap();

    assert!(png.starts_with(b"\x89PNG"), "not a PNG");
    assert!(png.len() > 100);
}

/// P5-24. The budget is 2 ms for one render; anything near it would show as a
/// stutter when the scale factor changes and the whole set is redrawn.
#[test]
fn a_single_render_stays_well_inside_the_budget() {
    use std::time::Instant;

    // Warm up, so the first allocation is not counted.
    let _ = render(spec(70.0, 32));

    let runs = 50;
    let started = Instant::now();
    for step in 0..runs {
        // A fresh renderer each time, or the cache would be doing the work.
        let _ = IconRenderer::new()
            .render(spec(f64::from(step % 11) * 10.0, 32))
            .unwrap();
    }
    let average = started.elapsed() / runs;

    assert!(
        average.as_micros() < 2_000,
        "a 32px render averaged {average:?}, against a 2ms budget"
    );
}

#[test]
fn the_cache_returns_the_same_icon_rather_than_redrawing() {
    let renderer = IconRenderer::new();
    let first = renderer.render(spec(70.0, 32)).unwrap();
    let second = renderer.render(spec(70.0, 32)).unwrap();

    assert!(
        std::sync::Arc::ptr_eq(&first, &second),
        "the second call rendered again instead of returning the cached icon"
    );
    assert_eq!(renderer.cached(), 1);

    renderer.render(spec(20.0, 32)).unwrap();
    assert_eq!(renderer.cached(), 2);
}

/// A scale factor that changes repeatedly changes the cache key each time, so
/// the bound is what keeps this from growing for as long as the process runs.
#[test]
fn the_cache_stays_bounded() {
    let renderer = IconRenderer::new();

    for size in IconSize::MIN..=(IconSize::MIN + IconRenderer::CAPACITY as u32 + 10) {
        renderer.render(spec(70.0, size)).unwrap();
    }

    assert!(renderer.cached() <= IconRenderer::CAPACITY);
    assert!(renderer.cached() > 0, "it should not have emptied itself");
}

#[test]
fn clearing_the_cache_empties_it() {
    let renderer = IconRenderer::new();
    renderer.render(spec(70.0, 32)).unwrap();
    assert_eq!(renderer.cached(), 1);

    renderer.clear();
    assert_eq!(renderer.cached(), 0);
}

/// Rendering is deterministic, which is what makes the cache sound and the
/// golden images meaningful.
#[test]
fn the_same_spec_always_draws_the_same_pixels() {
    let first = render(spec(40.0, 22));
    let second = render(spec(40.0, 22));

    assert_eq!(first.rgba(), second.rgba());
}
