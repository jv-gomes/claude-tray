# ADR-0005: Draw the tray icon at runtime with `tiny-skia`, without SVG

- **Status:** Accepted
- **Date:** 2026-08-01
- **Deciders:** @jv-gomes

## Context

The tray icon is a battery: a rounded body, a terminal nub, and a fill that drops in
ten discrete steps through four colour bands. It has to be produced for every
combination of fill level, colour band, theme, and staleness, at whatever pixel size
the platform asks for — 16 px on a Windows notification area, 22 px on a typical
Linux panel, 44 px on a HiDPI macOS menu bar at 2× scale.

Three constraints shape the choice.

**Sizes are not known ahead of time.** HiDPI scale factors are arbitrary and can
change while the application runs, when a window moves between monitors. Upscaling a
small raster to fit is exactly what makes a tray icon look wrong.

**The combinations multiply.** Eleven fill states × five colour states × four themes
× three overlays × stale-or-not, at six nominal sizes, is well past the point where
enumerating them is reasonable.

**The binary should stay small.** The performance budget allows 10 MB, and an icon
pipeline is not where that budget should go.

## Decision

We will **draw the icon procedurally with `tiny-skia`**, building paths in code and
rasterising to RGBA8 at the exact pixel size requested.

There is no SVG anywhere in the pipeline.

## Options considered

### Option A — Procedural drawing with `tiny-skia` (chosen)

- **Pros:** Any size renders exactly, so HiDPI is a parameter rather than a problem.
  The whole shape is a rounded rectangle, a small rectangle, and a clipped fill —
  roughly forty lines of path building, which is less code than an SVG template plus
  the code to patch its attributes. Four small pure-Rust dependencies. A single
  render measured well under the 2 ms budget, so no icon needs to be baked ahead of
  time. Geometry is expressed in fractions of the icon size, so proportions hold at
  every scale.
- **Cons:** The shape lives in Rust rather than in a file a designer can open. Any
  future icon that is genuinely illustrative — not composed of primitives — would
  need this decision revisited.

### Option B — Runtime SVG rasterisation with `resvg` + `tiny-skia`

- **Pros:** The shape lives in an SVG a designer can edit. Same rendering quality,
  since `resvg` rasterises through `tiny-skia` anyway.
- **Cons:** Pays for a full SVG implementation — `usvg`, `roxmltree`, plus font
  discovery and text shaping through `fontdb`, `rustybuzz`, and `ttf-parser` — to
  draw two rectangles and no text at all. Varying the fill level and colour means
  either string-templating the SVG on every render or walking the parsed tree to
  patch attributes; both are more fragile than calling a function with a
  `BatteryLevel`. The SVG layer buys editability we do not need for this shape.

### Option C — Pre-baked PNGs per state

- **Pros:** No rasteriser in the binary at all. Rendering cost is zero.
- **Cons:** Fails the HiDPI requirement outright: an arbitrary scale factor has no
  pre-baked size to match, and scaling a raster is what this is meant to avoid.
  Every combination has to be generated and committed, and the whole set has to be
  regenerated for any change to the shape or palette. Ships bytes for states a given
  user will never see.

### Option D — Hand-written rasteriser, no dependencies

- **Pros:** Nothing to depend on.
- **Cons:** Anti-aliased rounded rectangles at 16 px are where a naive rasteriser
  looks visibly wrong, and coverage-correct scanline filling is a real piece of work
  with subtle bugs. `tiny-skia` is four small crates for a problem that is solved.

## Consequences

### Positive

- HiDPI is handled by passing a different number; there is no upscaling path to get
  wrong, and no "nearest available size" fallback logic.
- A theme or palette change is a code change in one place, with no assets to
  regenerate and no chance of an asset drifting out of sync with the code.
- Golden-image tests can render every state on demand, so coverage of the icon
  matrix costs nothing to maintain.
- The macOS template variant — monochrome plus alpha — is another palette rather
  than a second set of assets.

### Negative

- Four dependencies (`tiny-skia`, `tiny-skia-path`, `arrayref`, `bytemuck`, plus
  `png` for encoding in tests and the icon-sheet tool) where a pre-baked pipeline
  would have none.
- `tiny-skia` is BSD-3-Clause, a third licence in a tree that is otherwise MIT and
  Apache-2.0. It is permissive and on the `cargo-deny` allowlist, but it does add an
  attribution obligation for binary distribution.
- The icon's appearance is only reviewable by rendering it. The `icon-sheet` example
  exists for exactly that reason and should be run whenever the drawing changes.

### Risks

- **`tiny-skia` uses `unsafe` internally.** Our own crates forbid it, and that
  guarantee does not extend to dependencies. The mitigation is that the crate is
  widely used, pure Rust, and the input to it here is entirely our own geometry —
  no untrusted file is ever parsed.
- **Rasteriser output can shift between versions.** Golden-image tests compare with a
  small per-pixel tolerance so an anti-aliasing improvement does not fail CI
  spuriously, while a real change to the shape still does.

## Revisit when

The icon needs artwork that is not composed of primitives, or a designer needs to
iterate on the shape without touching Rust — either would make the SVG pipeline in
Option B worth its weight.
