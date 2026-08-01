//! Golden-image tests: every state, compared against committed references.
//!
//! These catch changes nobody meant to make. The unit tests in the crate assert
//! properties — the fill is monotonic, the template carries no colour — which is
//! where the meaning lives; these assert that the pixels have not moved, which
//! is what catches a geometry tweak quietly reshaping a state nobody was
//! looking at.
//!
//! Regenerate after an intended change, then **look at the diff before
//! committing it**:
//!
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p claude-tray-icon --test golden
//! cargo run --example icon-sheet -p claude-tray-icon
//! ```

// An integration test is its own crate, so the library's test-only allowances do
// not reach it. The same reasoning applies: a panicking helper is the right way
// for a test to fail.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use claude_tray_core::{BatteryLevel, ColorState, Percent, Thresholds};
use claude_tray_icon::{Appearance, Icon, IconRenderer, IconSize, IconSpec, Overlay, Rgb, Theme};

/// How far a channel may drift before a difference counts as real.
///
/// Anti-aliasing is where a rasteriser update shows up, and a one- or two-level
/// shift along an edge is not a change to the icon. A moved edge, a wrong
/// colour, or a missing element all move far more than this.
const CHANNEL_TOLERANCE: u8 = 4;

/// How many pixels may differ by more than [`CHANNEL_TOLERANCE`], as a fraction
/// of the icon.
///
/// Not zero, because a rasteriser can legitimately place an edge pixel
/// differently; small enough that a shifted shape fails, since even a one-pixel
/// move affects far more than this share of a 64 px icon.
const PIXEL_TOLERANCE: f64 = 0.02;

fn goldens() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn updating() -> bool {
    std::env::var_os("UPDATE_GOLDEN").is_some()
}

/// Every state worth pinning, with the name its reference is filed under.
fn cases() -> Vec<(&'static str, IconSpec)> {
    let base = |remaining: f64, size: u32| {
        IconSpec::from_percent(
            Percent::new(remaining).unwrap(),
            Thresholds::default(),
            Theme::Dark,
            IconSize::new(size).unwrap(),
        )
    };
    let reference = base(30.0, 64);

    let mut cases: Vec<(&'static str, IconSpec)> = vec![
        // One per colour band, and both ends of the range.
        ("dark-100", base(100.0, 64)),
        ("dark-075", base(75.0, 64)),
        ("dark-050", base(50.0, 64)),
        ("dark-030", base(30.0, 64)),
        ("dark-010", base(10.0, 64)),
        ("dark-000", base(0.0, 64)),
        (
            "light-050",
            IconSpec {
                theme: Theme::Light,
                ..base(50.0, 64)
            },
        ),
        // The states that are not a reading.
        (
            "unknown",
            IconSpec {
                color_state: ColorState::Unknown,
                level: BatteryLevel::EMPTY,
                ..reference
            },
        ),
        (
            "loading",
            IconSpec {
                overlay: Overlay::Loading,
                ..reference
            },
        ),
        (
            "error",
            IconSpec {
                overlay: Overlay::Error,
                ..reference
            },
        ),
        (
            "stale",
            IconSpec {
                stale: true,
                ..reference
            },
        ),
        // Every theme.
        (
            "theme-monochrome-dark",
            IconSpec {
                theme: Theme::Monochrome(Appearance::Dark),
                ..reference
            },
        ),
        (
            "theme-monochrome-light",
            IconSpec {
                theme: Theme::Monochrome(Appearance::Light),
                ..reference
            },
        ),
        (
            "theme-template",
            IconSpec {
                theme: Theme::Template,
                ..reference
            },
        ),
        (
            "theme-accent",
            IconSpec {
                theme: Theme::Accent(Rgb::hex(0x00_7A_CC)),
                ..reference
            },
        ),
        (
            "theme-colorblindsafe",
            IconSpec {
                theme: Theme::ColorBlindSafe(Appearance::Dark),
                ..reference
            },
        ),
    ];

    // The small sizes, where rounding decides whether the shape survives.
    cases.push(("size-16", base(70.0, 16)));
    cases.push(("size-22", base(70.0, 22)));
    cases.push(("size-24", base(70.0, 24)));

    cases
}

/// Compares two icons, returning how many pixels differ beyond the tolerance.
fn differing_pixels(rendered: &Icon, reference: &Icon) -> Option<usize> {
    if rendered.width() != reference.width() || rendered.height() != reference.height() {
        return None;
    }

    Some(
        rendered
            .rgba()
            .chunks_exact(4)
            .zip(reference.rgba().chunks_exact(4))
            .filter(|(a, b)| {
                a.iter()
                    .zip(b.iter())
                    .any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE)
            })
            .count(),
    )
}

/// A pixel count as a fraction of the icon.
///
/// An icon is at most 512 x 512, so the count is far below the point where an
/// `f64` stops representing integers exactly.
#[allow(clippy::cast_precision_loss)]
fn share_of(differing: usize, total: f64) -> f64 {
    differing as f64 / total
}

/// Decodes a reference PNG back into an icon.
fn load(path: &Path) -> Option<Icon> {
    let file = std::fs::File::open(path).ok()?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);

    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;

    buffer.truncate(info.buffer_size());
    Icon::from_rgba(info.width, info.height, buffer)
}

#[test]
fn every_state_matches_its_reference() {
    let renderer = IconRenderer::new();
    let directory = goldens();
    let mut failures = Vec::new();

    if updating() {
        std::fs::create_dir_all(&directory).expect("could not create the golden directory");
    }

    for (name, spec) in cases() {
        let icon = renderer.render(spec).expect("render failed");
        let path = directory.join(format!("{name}.png"));

        if updating() {
            std::fs::write(&path, icon.to_png().expect("could not encode"))
                .expect("could not write");
            continue;
        }

        let Some(reference) = load(&path) else {
            failures.push(format!(
                "{name}: no reference at {}; run with UPDATE_GOLDEN=1 to create it",
                path.display()
            ));
            continue;
        };

        let Some(differing) = differing_pixels(&icon, &reference) else {
            failures.push(format!(
                "{name}: rendered {}x{} against a reference of {}x{}",
                icon.width(),
                icon.height(),
                reference.width(),
                reference.height()
            ));
            continue;
        };

        let total = f64::from(icon.width() * icon.height());
        let share = share_of(differing, total);

        if share > PIXEL_TOLERANCE {
            failures.push(format!(
                "{name}: {differing} of {total:.0} pixels differ ({:.1}%), above the {:.0}% \
                 tolerance",
                share * 100.0,
                PIXEL_TOLERANCE * 100.0
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "golden images differ:\n  {}\n\nIf the change was intended, regenerate with \
         UPDATE_GOLDEN=1 and inspect the result before committing.",
        failures.join("\n  ")
    );
}

/// The tolerance has to be loose enough to survive a rasteriser update and tight
/// enough to catch a real change. A one-step difference in fill level is the
/// smallest change that matters, so it must fail.
#[test]
fn the_tolerance_still_catches_a_real_change() {
    let renderer = IconRenderer::new();
    let base = IconSpec::from_percent(
        Percent::new(50.0).unwrap(),
        Thresholds::default(),
        Theme::Dark,
        IconSize::new(64).unwrap(),
    );
    let one_step_lower = IconSpec::from_percent(
        Percent::new(40.0).unwrap(),
        Thresholds::default(),
        Theme::Dark,
        IconSize::new(64).unwrap(),
    );

    let a = renderer.render(base).unwrap();
    let b = renderer.render(one_step_lower).unwrap();

    let differing = differing_pixels(&a, &b).unwrap();
    let share = share_of(differing, f64::from(a.width() * a.height()));

    assert!(
        share > PIXEL_TOLERANCE,
        "a whole fill step differs by only {:.2}% of the icon, so the tolerance would \
         let a real change through",
        share * 100.0
    );
}

#[test]
fn an_icon_compares_equal_to_itself() {
    let renderer = IconRenderer::new();
    let spec = IconSpec::from_percent(
        Percent::new(50.0).unwrap(),
        Thresholds::default(),
        Theme::Dark,
        IconSize::new(32).unwrap(),
    );

    let icon = renderer.render(spec).unwrap();
    assert_eq!(differing_pixels(&icon, &icon), Some(0));
}
