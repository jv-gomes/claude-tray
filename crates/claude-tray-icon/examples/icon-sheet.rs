//! Dumps every icon state to PNG, so the drawing can be looked at.
//!
//! The icon is only reviewable by rendering it — that is the cost of drawing in
//! code rather than in a file — so run this whenever the geometry or the palette
//! changes.
//!
//! ```sh
//! cargo run --example icon-sheet -p claude-tray-icon
//! ```
//!
//! Writes to `target/icon-sheet/`, or to a directory given as the first
//! argument.

use std::path::PathBuf;

use claude_tray_core::{BatteryLevel, ColorState, Percent, Thresholds};
use claude_tray_icon::{Appearance, IconRenderer, IconSize, IconSpec, Overlay, Rgb, Theme};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/icon-sheet"), PathBuf::from);
    std::fs::create_dir_all(&directory)?;

    let renderer = IconRenderer::new();
    let mut written = 0;

    let mut write = |name: String, spec: IconSpec| -> Result<(), Box<dyn std::error::Error>> {
        let icon = renderer.render(spec)?;
        std::fs::write(directory.join(format!("{name}.png")), icon.to_png()?)?;
        written += 1;
        Ok(())
    };

    // Every fill level, in both default themes, at a size where the shape is
    // legible on a page.
    for (theme, label) in [(Theme::Dark, "dark"), (Theme::Light, "light")] {
        for percent in [
            100.0, 90.0, 80.0, 70.0, 60.0, 50.0, 40.0, 30.0, 20.0, 10.0, 0.0,
        ] {
            let spec = IconSpec::from_percent(
                Percent::new(percent)?,
                Thresholds::default(),
                theme,
                IconSize::new(64)?,
            );
            write(format!("{label}-{percent:03.0}"), spec)?;
        }
    }

    // The states that are not a reading.
    let base = IconSpec::from_percent(
        Percent::new(30.0)?,
        Thresholds::default(),
        Theme::Dark,
        IconSize::new(64)?,
    );

    write(
        "state-stale".to_owned(),
        IconSpec {
            stale: true,
            ..base
        },
    )?;
    write(
        "state-loading".to_owned(),
        IconSpec {
            overlay: Overlay::Loading,
            ..base
        },
    )?;
    write(
        "state-error".to_owned(),
        IconSpec {
            overlay: Overlay::Error,
            ..base
        },
    )?;
    write(
        "state-unknown".to_owned(),
        IconSpec {
            color_state: ColorState::Unknown,
            level: BatteryLevel::EMPTY,
            ..base
        },
    )?;

    // Every theme, at a level where the fill is visible.
    for (theme, label) in [
        (Theme::Monochrome(Appearance::Dark), "theme-monochrome-dark"),
        (
            Theme::Monochrome(Appearance::Light),
            "theme-monochrome-light",
        ),
        (Theme::Template, "theme-template"),
        (Theme::Accent(Rgb::hex(0x00_7A_CC)), "theme-accent"),
        (
            Theme::ColorBlindSafe(Appearance::Dark),
            "theme-colorblindsafe",
        ),
    ] {
        write(label.to_owned(), IconSpec { theme, ..base })?;
    }

    // The sizes a real panel asks for, to check the shape survives being small.
    for nominal in IconSize::NOMINAL {
        let spec = IconSpec::from_percent(
            Percent::new(70.0)?,
            Thresholds::default(),
            Theme::Dark,
            IconSize::new(nominal)?,
        );
        write(format!("size-{nominal:02}"), spec)?;
    }

    println!("wrote {written} icons to {}", directory.display());
    Ok(())
}
