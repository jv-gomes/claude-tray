//! Drawing the battery.

use tiny_skia::{
    FillRule, Paint, PathBuilder, Pixmap, PixmapMut, Rect, Stroke, StrokeDash, Transform,
};

use claude_tray_core::BatteryLevel;

use crate::{Icon, IconSpec, Overlay, Palette, Rgb};

/// How much of the icon's width the terminal nub takes.
const NUB_WIDTH: f32 = 0.075;

/// The gap between the battery body and the nub.
const NUB_GAP: f32 = 0.03;

/// The margin from the canvas edge to the battery body.
const MARGIN: f32 = 0.07;

/// How tall the battery body is, as a fraction of the canvas.
const BODY_HEIGHT: f32 = 0.52;

/// How tall the nub is, relative to the body.
const NUB_HEIGHT: f32 = 0.45;

/// The gap between the battery outline and the fill inside it, as a fraction of
/// the icon size.
///
/// Proportional to the icon rather than to the stroke. The stroke is rounded to
/// whole pixels so it stays crisp, which makes it jump from 1 px to 2 px between
/// 22 px and 24 px — and a padding derived from it would jump with it, drawing
/// the same percentage visibly smaller at one of two adjacent sizes.
const FILL_PADDING: f32 = 0.075;

/// Alpha applied to the whole icon when the reading behind it is stale.
const STALE_ALPHA: f32 = 0.45;

/// Alpha for the unfilled part of the battery, so it reads as a container
/// rather than as a bar floating in space.
const EMPTY_ALPHA: f32 = 0.22;

/// Renders a specification to pixels.
///
/// Geometry is expressed in fractions of the icon size and multiplied up, so the
/// proportions hold at 16 px and at 64 px without a table of per-size tweaks.
pub(crate) fn draw(spec: IconSpec) -> Option<Icon> {
    let size = spec.size.pixels();
    let mut pixmap = Pixmap::new(size, size)?;

    let layout = Layout::for_size(spec.size.as_f32())?;
    let palette = spec.theme.palette(spec.color_state);
    let alpha = if spec.stale { STALE_ALPHA } else { 1.0 };

    let mut canvas = pixmap.as_mut();

    draw_shell(&mut canvas, spec, &layout, palette, alpha);

    if spec.overlay == Overlay::Loading {
        draw_loading(&mut canvas, layout.body, palette, alpha);
    } else if !spec.is_outlined() {
        draw_fill(&mut canvas, spec.level, &layout, palette, alpha);
    }

    if spec.overlay == Overlay::Error {
        draw_error_badge(&mut canvas, layout.size);
    }

    Some(Icon::from_pixmap(&pixmap))
}

/// Where everything sits, worked out once from the icon size.
struct Layout {
    /// The icon's edge length, in pixels.
    size: f32,
    /// The battery body, as the centre line of its outline.
    body: Rect,
    /// How thick that outline is, rounded to whole pixels so it stays crisp.
    stroke: f32,
}

impl Layout {
    fn for_size(size: f32) -> Option<Self> {
        let stroke = (size / 16.0).max(1.0).round();

        // Stroked paths are centred on the path, so insetting by half the
        // stroke is what keeps the outline inside the canvas rather than
        // clipped by it.
        let inset = stroke / 2.0;
        let body_width = size * (1.0 - MARGIN * 2.0 - NUB_WIDTH - NUB_GAP);

        Some(Self {
            size,
            body: Rect::from_xywh(
                size * MARGIN + inset,
                (size - size * BODY_HEIGHT) / 2.0 + inset,
                body_width - stroke,
                size * BODY_HEIGHT - stroke,
            )?,
            stroke,
        })
    }
}

/// The battery body and its terminal nub.
fn draw_shell(
    canvas: &mut PixmapMut<'_>,
    spec: IconSpec,
    layout: &Layout,
    palette: Palette,
    alpha: f32,
) {
    let body = layout.body;
    let radius = body.height() * 0.28;
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };

    // At exactly nothing left there is no fill to carry the colour, so the
    // shell takes it. Otherwise the most alarming state would draw as the least
    // alarming icon: an ordinary empty battery, indistinguishable from one that
    // simply has no reading.
    let shell = if spec.level == BatteryLevel::EMPTY && !spec.is_outlined() {
        palette.fill
    } else {
        palette.shell
    };
    paint.set_color(color(shell, alpha));

    let mut stroke = Stroke {
        width: layout.stroke,
        ..Stroke::default()
    };

    if spec.is_outlined() {
        // A dashed shell reads as "nothing to report". An empty solid battery
        // would read as "nothing left", which is the opposite.
        let dash = (body.width() / 8.0).max(1.5);
        stroke.dash = StrokeDash::new(vec![dash, dash * 0.7], 0.0);
    }

    if let Some(path) = rounded_rect(body, radius) {
        canvas.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    // The nub sits outside the body, centred vertically.
    let nub_height = body.height() * NUB_HEIGHT;
    if let Some(nub) = Rect::from_xywh(
        body.right() + layout.stroke / 2.0 + layout.size * NUB_GAP,
        body.y() + (body.height() - nub_height) / 2.0,
        layout.size * NUB_WIDTH,
        nub_height,
    ) && let Some(path) = rounded_rect(nub, nub_height * 0.3)
    {
        canvas.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

/// The fill, snapped to the ten discrete steps the level already carries.
fn draw_fill(
    canvas: &mut PixmapMut<'_>,
    level: BatteryLevel,
    layout: &Layout,
    palette: Palette,
    alpha: f32,
) {
    let body = layout.body;

    // Still has to clear the stroke, which is centred on the body path, so the
    // proportional padding is a floor rather than the whole story.
    let padding = (layout.size * FILL_PADDING).max(layout.stroke.mul_add(0.5, 1.0));
    let Some(track) = Rect::from_xywh(
        body.x() + padding,
        body.y() + padding,
        body.width() - padding * 2.0,
        body.height() - padding * 2.0,
    ) else {
        return;
    };

    let radius = track.height() * 0.22;
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };

    // The unfilled part first, so the battery reads as a container even at zero.
    paint.set_color(color(palette.empty, alpha * EMPTY_ALPHA));
    if let Some(path) = rounded_rect(track, radius) {
        canvas.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    let filled = f32::from(level.filled()) / f32::from(BatteryLevel::SEGMENTS);
    if filled <= 0.0 {
        return;
    }

    paint.set_color(color(palette.fill, alpha));
    if let Some(fill) =
        Rect::from_xywh(track.x(), track.y(), track.width() * filled, track.height())
        && let Some(path) = rounded_rect(fill, radius.min(fill.width() / 2.0))
    {
        canvas.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

/// Three dots inside the shell: something is happening, nothing is known yet.
///
/// Static rather than animated. A tray icon that animates costs a redraw per
/// frame for the whole time the first fetch takes, which is the opposite of the
/// idle budget.
fn draw_loading(canvas: &mut PixmapMut<'_>, body: Rect, palette: Palette, alpha: f32) {
    let radius = (body.height() * 0.13).max(0.75);
    let spacing = body.width() / 4.0;
    let centre_y = body.y() + body.height() / 2.0;

    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paint.set_color(color(palette.empty, alpha * 0.75));

    for index in 1..=3 {
        // `index` is 1..=3, so the conversion is exact.
        let offset = spacing * f32::from(u8::try_from(index).unwrap_or(1));
        if let Some(dot) = circle(body.x() + offset, centre_y, radius) {
            canvas.fill_path(&dot, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
}

/// A warning badge in the bottom-right corner.
///
/// Drawn over the battery rather than replacing it, so an error that arrives
/// while a reading is on screen does not throw the reading away.
fn draw_error_badge(canvas: &mut PixmapMut<'_>, size: f32) {
    let radius = size * 0.24;
    let centre = size - radius - size * 0.04;

    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };

    // The badge is always warning-coloured, even in a monochrome theme: this is
    // the one place where being noticed matters more than being unobtrusive.
    paint.set_color(color(Rgb::hex(0xD5_2B_1E), 1.0));
    if let Some(disc) = circle(centre, centre, radius) {
        canvas.fill_path(
            &disc,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    // White on the red badge, whatever the theme: this mark has to read on both
    // panels, and a themed colour would vanish into the badge on one of them.
    paint.set_color(color(Rgb::hex(0xFF_FF_FF), 1.0));

    let bar_width = (radius * 0.22).max(1.0);
    if let Some(bar) = Rect::from_xywh(
        centre - bar_width / 2.0,
        centre - radius * 0.5,
        bar_width,
        radius * 0.62,
    ) {
        canvas.fill_rect(bar, &paint, Transform::identity(), None);
    }

    if let Some(dot) = circle(centre, centre + radius * 0.42, bar_width * 0.62) {
        canvas.fill_path(&dot, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// A rounded rectangle as a path.
fn rounded_rect(rect: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let radius = radius
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0)
        .max(0.0);
    let (left, top) = (rect.x(), rect.y());
    let (right, bottom) = (rect.right(), rect.bottom());

    let mut builder = PathBuilder::new();
    builder.move_to(left + radius, top);
    builder.line_to(right - radius, top);
    builder.quad_to(right, top, right, top + radius);
    builder.line_to(right, bottom - radius);
    builder.quad_to(right, bottom, right - radius, bottom);
    builder.line_to(left + radius, bottom);
    builder.quad_to(left, bottom, left, bottom - radius);
    builder.line_to(left, top + radius);
    builder.quad_to(left, top, left + radius, top);
    builder.close();
    builder.finish()
}

/// A circle as a path.
fn circle(cx: f32, cy: f32, radius: f32) -> Option<tiny_skia::Path> {
    PathBuilder::from_circle(cx, cy, radius.max(0.1))
}

/// An opaque colour at the given alpha.
fn color(rgb: Rgb, alpha: f32) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(
        rgb.r,
        rgb.g,
        rgb.b,
        // Alpha is always a fraction of 255, so the conversion is exact.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            (alpha.clamp(0.0, 1.0) * 255.0).round() as u8
        },
    )
}
