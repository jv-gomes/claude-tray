//! Rendered pixels, and the cache that avoids re-rendering them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tiny_skia::Pixmap;

use crate::{IconSpec, render};

/// A rendered icon, as the tray platforms want it.
///
/// **Straight alpha, not premultiplied.** `tiny-skia` composites in
/// premultiplied form and every tray backend expects the opposite, so the
/// conversion happens once here. Skipping it looks almost right — solid areas
/// are unaffected — and shows up as dark fringes on every anti-aliased edge,
/// which at 16 px is most of the icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Icon {
    /// Converts a rendered pixmap into straight-alpha RGBA8.
    pub(crate) fn from_pixmap(pixmap: &Pixmap) -> Self {
        let mut rgba = Vec::with_capacity(pixmap.data().len());

        for pixel in pixmap.pixels() {
            let straight = pixel.demultiply();
            rgba.extend_from_slice(&[
                straight.red(),
                straight.green(),
                straight.blue(),
                straight.alpha(),
            ]);
        }

        Self {
            width: pixmap.width(),
            height: pixmap.height(),
            rgba,
        }
    }

    /// Rebuilds an icon from a straight-alpha RGBA buffer.
    ///
    /// For reading a golden image back to compare against. Returns `None` if the
    /// buffer is not exactly four bytes per pixel.
    #[must_use]
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        if rgba.len() != (width as usize) * (height as usize) * 4 {
            return None;
        }

        Some(Self {
            width,
            height,
            rgba,
        })
    }

    /// The icon's width in physical pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The icon's height in physical pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The pixels, four bytes each, row-major, straight alpha.
    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Consumes the icon and returns its buffer.
    #[must_use]
    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }

    /// Returns one pixel as `(r, g, b, a)`, for tests and diffing.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let index = ((y * self.width + x) * 4) as usize;
        let pixel = self.rgba.get(index..index + 4)?;
        Some((
            *pixel.first()?,
            *pixel.get(1)?,
            *pixel.get(2)?,
            *pixel.get(3)?,
        ))
    }

    /// Encodes the icon as a PNG.
    ///
    /// For the `icon-sheet` tool and for writing golden images; the tray itself
    /// never wants a PNG.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer cannot be encoded, which should not
    /// happen for a buffer this type produced.
    pub fn to_png(&self) -> Result<Vec<u8>, PngError> {
        let mut pixmap = Pixmap::new(self.width, self.height).ok_or(PngError)?;

        for (destination, source) in pixmap
            .pixels_mut()
            .iter_mut()
            .zip(self.rgba.chunks_exact(4))
        {
            let (Some(r), Some(g), Some(b), Some(a)) =
                (source.first(), source.get(1), source.get(2), source.get(3))
            else {
                return Err(PngError);
            };

            *destination = tiny_skia::ColorU8::from_rgba(*r, *g, *b, *a).premultiply();
        }

        pixmap.encode_png().map_err(|_| PngError)
    }
}

/// The icon could not be encoded as a PNG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("could not encode the icon as a PNG")]
pub struct PngError;

/// Renders icons, remembering what it has already drawn.
///
/// The tray asks for an icon far more often than the underlying state changes —
/// every refresh, whether or not the number moved — and a redraw of an identical
/// icon is pure waste. The cache is keyed on the whole specification, which is
/// sound because two equal specs produce identical pixels by construction.
#[derive(Debug, Default)]
pub struct IconRenderer {
    cache: Mutex<HashMap<IconSpec, Arc<Icon>>>,
}

impl IconRenderer {
    /// How many icons to hold before starting over.
    ///
    /// The working set is one icon per state the tray actually passes through at
    /// one size — a couple of dozen at most. The bound exists so that a scale
    /// factor changing repeatedly, which changes the size and therefore the key,
    /// cannot grow this without limit.
    pub const CAPACITY: usize = 64;

    /// Creates an empty renderer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the icon for a specification, rendering it if it is new.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the size could not be allocated.
    pub fn render(&self, spec: IconSpec) -> Result<Arc<Icon>, RenderError> {
        let mut cache = match self.cache.lock() {
            Ok(guard) => guard,
            // A poisoned lock means another thread panicked while rendering.
            // The cache holds nothing but pixels, so it is safe to carry on
            // rather than propagate someone else's panic into the tray.
            Err(poisoned) => poisoned.into_inner(),
        };

        if let Some(icon) = cache.get(&spec) {
            return Ok(Arc::clone(icon));
        }

        let icon = Arc::new(render::draw(spec).ok_or(RenderError { spec })?);

        // Clearing rather than evicting one entry: the working set is small
        // enough that refilling it costs a few renders, and a proper eviction
        // policy would be more machinery than the problem deserves.
        if cache.len() >= Self::CAPACITY {
            cache.clear();
        }
        cache.insert(spec, Arc::clone(&icon));

        Ok(icon)
    }

    /// How many icons are currently held.
    #[must_use]
    pub fn cached(&self) -> usize {
        match self.cache.lock() {
            Ok(guard) => guard.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        }
    }

    /// Drops every cached icon.
    ///
    /// Call this when the palette itself changes — a theme switch — since the
    /// specification changes with it and the old entries would only sit there.
    pub fn clear(&self) {
        match self.cache.lock() {
            Ok(mut guard) => guard.clear(),
            Err(poisoned) => poisoned.into_inner().clear(),
        }
    }
}

/// An icon could not be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("could not render an icon at {}px", spec.size.pixels())]
pub struct RenderError {
    /// What was being drawn.
    pub spec: IconSpec,
}
