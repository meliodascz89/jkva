//! `jkva-text` — shaping, measurement and glyph rasterisation.
//!
//! This is the only crate in JKva that talks to fonts. It turns a string plus a
//! [`TextStyle`] into positioned glyphs ([`ShapedText`]) and turns a single
//! glyph into coverage bits ([`GlyphBitmap`]); everything else — line breaking,
//! bidi, inline boxes, the glyph atlas — belongs to `jkva-layout` and
//! `jkva-gfx`.
//!
//! # Pipeline
//!
//! 1. **Selection** — [`FontCache`] asks `fontdb` for the face matching the
//!    requested family, weight and slant. System fonts are scanned once, in
//!    [`FontCache::new`].
//! 2. **Shaping** — `harfrust` substitutes and positions glyphs. The requested
//!    scale is 26.6 fixed point (64 units per CSS pixel) so advances keep
//!    1/64 px precision; positions are converted back to `f32` pixels.
//! 3. **Rasterisation** — `swash` renders one glyph into an 8-bit coverage
//!    bitmap that `jkva-gfx` copies into its atlas.
//!
//! # Units and coordinates
//!
//! Sizes are CSS pixels per em. Glyph `x`/`y` offsets are relative to the pen
//! position on the baseline, with **y positive upwards** (the font convention);
//! paint code flips it. [`ShapedText::ascent`] is the distance above the
//! baseline and [`ShapedText::descent`] below it; both are positive, so
//! `ascent + descent` is the natural line height of the run at `line-height: 1`
//! (CSS multiplies by `line-height` in layout).
//!
//! # Fallback behaviour
//!
//! Fonts are a system resource, so every failure is recoverable and reported
//! through [`FontCache::warnings`]:
//!
//! | Situation | Behaviour |
//! |---|---|
//! | Requested family is not installed | Falls back to the system `sans-serif`, then `serif`, then any installed face; one warning names the requested family and the one used. |
//! | No fonts at all | Shaping returns an empty [`ShapedText`] (zero width, zero metrics) and one warning. Nothing panics. |
//! | Face bytes are unreadable or unsupported | The request is answered with an empty run and one warning. |
//! | Shaping itself rejects the input | Empty run (no glyphs, zero width) and one warning; metrics are still filled in. |
//! | Glyph has no outline/bitmap | [`FontCache::rasterize`] returns `None`. |
//! | Degenerate style (NaN, 0, negative size) | Size is clamped, see [`TextStyle`]. |
//!
//! Warnings are deduplicated: shaping runs in a hot path and one missing family
//! must not grow the list without bound. They are diagnostics, not state — an
//! engine may read them once after layout and show them in `about:` pages.
//!
//! # Not implemented yet
//!
//! - Colour glyphs (COLRv1, `sbix`, CBDT). Outline fonts render today; emoji fall
//!   back to a monochrome outline or no bitmap. ADR-015 lists colour glyphs as a
//!   requirement, so this is a scheduled gap rather than a design choice.
//! - Font fallback per character: one face per run. A run in a font that lacks
//!   a character shows `.notdef` instead of borrowing the glyph from another
//!   font. Per-cluster fallback belongs with line breaking.
//! - Variable-font axes, `trak` tracking, hinting, subpixel positioning and
//!   gamma correction. Gamma and blending are applied when compositing.
//! - `font-stretch`, synthetic bold/italic and `font-variant-*`.
//!
//! # Invariants
//!
//! - No `unsafe` in this crate (`#![forbid(unsafe_code)]`).
//! - Shaping, measurement and rasterisation never panic on hostile text, an
//!   empty string, a missing font or a stale glyph id.
//! - Glyph bytes are loaded at most once per face and shared between shaping
//!   and rasterisation.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod font;
mod raster;

use std::collections::HashMap;

use fontdb::Database;
use harfrust::{Buffer, ShapeOptions, ShaperFont};
use swash::scale::ScaleContext;

use crate::font::{LoadedFace, Resolution};

/// Size in pixels used when a [`TextStyle`] carries no usable size.
const DEFAULT_SIZE: f32 = 16.0;

/// Smallest size we shape or rasterise, in pixels.
///
/// Guards the fixed-point conversions below and keeps a `font-size: 0` (which
/// CSS allows as a legal, if useless, value) from producing zero-scale math.
const MIN_SIZE: f32 = 0.5;

/// Largest size we shape or rasterise, in pixels. Well past any real style.
const MAX_SIZE: f32 = 1024.0;

/// Shaping scale: 26.6 fixed point, i.e. 64 units per CSS pixel.
const SCALE_PER_PIXEL: f32 = 64.0;

/// Style of one text run, in CSS terms.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// CSS `font-family` name, or a generic family (`serif`, `sans-serif`,
    /// `monospace`, `cursive`, `fantasy`, `system-ui`).
    pub family: String,

    /// Size in CSS pixels per em.
    ///
    /// Shaping and rasterisation clamp this to `0.5..=1024.0`; a non-finite or
    /// non-positive value is treated as 16 px, never as "invisible text".
    pub size: f32,

    /// CSS `font-weight`, clamped to `1..=1000` when a face is selected.
    pub weight: u16,

    /// `true` selects an italic (or, failing that, oblique) face.
    pub italic: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: "Helvetica".to_owned(),
            size: DEFAULT_SIZE,
            weight: 400,
            italic: false,
        }
    }
}

impl TextStyle {
    /// Creates a style from its parts.
    ///
    /// Equivalent to building the struct literally; `size` is sanitised when it
    /// is used, not here.
    #[must_use]
    pub fn new(family: impl Into<String>, size: f32, weight: u16, italic: bool) -> Self {
        Self {
            family: family.into(),
            size,
            weight,
            italic,
        }
    }

    /// The size in pixels that shaping and rasterisation will actually use.
    ///
    /// Guarantees a finite value inside `MIN_SIZE..=MAX_SIZE`.
    fn effective_size(&self) -> f32 {
        if self.size.is_finite() && self.size > 0.0 {
            self.size.clamp(MIN_SIZE, MAX_SIZE)
        } else {
            DEFAULT_SIZE
        }
    }
}

/// One glyph placed by the shaper.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShapedGlyph {
    /// Glyph identifier in the face that shaped the run.
    ///
    /// Identifiers above `u16::MAX` (only produced by custom glyph callbacks,
    /// never by system fonts) saturate.
    pub id: u16,

    /// Horizontal offset from the pen position, in pixels. Non-zero for marks
    /// and for kerning expressed as offsets.
    pub x: f32,

    /// Vertical offset from the baseline, in pixels, positive upwards.
    pub y: f32,

    /// Horizontal advance in pixels. Always non-negative for horizontal text.
    pub advance: f32,
}

/// A shaped, measured text run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapedText {
    /// Glyphs in visual order.
    pub glyphs: Vec<ShapedGlyph>,

    /// Total advance width in pixels.
    ///
    /// Exactly the sum of [`ShapedGlyph::advance`] over [`Self::glyphs`], so
    /// layout may use either representation.
    pub width: f32,

    /// Distance from the baseline to the top of the line, in pixels, positive.
    pub ascent: f32,

    /// Distance from the baseline to the bottom of the line, in pixels,
    /// positive (downwards).
    pub descent: f32,
}

/// One rasterised glyph: an 8-bit coverage bitmap.
///
/// `alpha[y * width + x]` is the coverage of the pixel at column `x`, row `y`,
/// where the bitmap origin sits at `(left, top)` relative to the glyph's
/// baseline origin and `top` is measured upwards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphBitmap {
    /// Bitmap width in pixels.
    pub width: u32,

    /// Bitmap height in pixels.
    pub height: u32,

    /// Horizontal offset of the bitmap's left edge from the glyph origin.
    pub left: i32,

    /// Vertical offset of the bitmap's top edge from the baseline, positive
    /// upwards (so it is usually negative for glyphs that sit on the baseline).
    pub top: i32,

    /// Coverage values, row-major, `0` = transparent, `255` = opaque.
    pub alpha: Vec<u8>,
}

/// The parts of a [`TextStyle`] that select a face, normalised for caching.
#[derive(Clone, PartialEq, Eq, Hash)]
struct StyleKey {
    /// Family name, trimmed and lowercased.
    family: String,
    /// Weight, already clamped to the CSS range.
    weight: u16,
    /// Italic flag.
    italic: bool,
}

impl StyleKey {
    /// Builds the cache key for `style`.
    fn new(style: &TextStyle) -> Self {
        Self {
            family: style.family.trim().to_lowercase(),
            weight: style.weight.clamp(1, 1000),
            italic: style.italic,
        }
    }
}

/// Discovered system fonts plus everything derived from them.
///
/// Construction scans the system font directories once and takes tens of
/// milliseconds; create one `FontCache` per engine (or per thread) and reuse
/// it. Font files are read lazily, on the first request for a face, and loaded
/// faces stay resident — the cache is the engine's font lifetime.
///
/// # Example
///
/// ```
/// use jkva_text::{FontCache, TextStyle};
///
/// let mut fonts = FontCache::new();
/// let style = TextStyle::default();
/// let run = fonts.shape("Hello", &style);
/// assert_eq!(run.glyphs.len(), 5);
/// assert!(run.width > 0.0);
/// ```
/// Key of one shaped run: the text plus every style input that affects shaping.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ShapeKey {
    text: String,
    size_bits: u32,
    weight: u16,
    family: String,
    italic: bool,
}

impl ShapeKey {
    fn new(text: &str, style: &TextStyle) -> Self {
        Self {
            text: text.to_string(),
            size_bits: style.effective_size().to_bits(),
            weight: style.weight,
            family: style.family.clone(),
            italic: style.italic,
        }
    }
}

/// Upper bound on cached shaped runs before the cache is cleared.
///
/// Clearing wholesale is a deliberate, simple policy: a page with thousands of
/// unique strings should not grow the cache without bound, and re-shaping after a
/// clear is correct, only slower.
pub const MAX_SHAPED_CACHE: usize = 4096;

/// A cache of system fonts, loaded faces, resolved styles and shaped runs.
pub struct FontCache {
    /// The system font database.
    database: Database,
    /// Faces whose bytes have been loaded, keyed by `fontdb` id.
    faces: HashMap<fontdb::ID, LoadedFace>,
    /// Face resolution per style key; `None` caches a known miss.
    resolutions: HashMap<StyleKey, Option<fontdb::ID>>,
    /// Deduplicated fallback diagnostics.
    warnings: Vec<String>,
    /// Scratch buffers and the hinting cache used by `swash`.
    raster_context: ScaleContext,
    /// Shaped runs keyed by text and style.
    shaped: HashMap<ShapeKey, ShapedText>,
}

impl FontCache {
    /// Scans the system fonts and returns an empty cache.
    ///
    /// Never fails: a machine with no readable fonts yields a cache that shapes
    /// everything to an empty run and reports it through [`Self::warnings`].
    #[must_use]
    pub fn new() -> Self {
        let mut database = Database::new();
        database.load_system_fonts();
        Self {
            database,
            faces: HashMap::new(),
            resolutions: HashMap::new(),
            warnings: Vec::new(),
            raster_context: ScaleContext::new(),
            shaped: HashMap::new(),
        }
    }

    /// Shapes `text` and measures the resulting run.
    ///
    /// The returned [`ShapedText::width`] is exactly the sum of the glyph
    /// advances. An empty string is a zero-width run with the face's line
    /// metrics still filled in.
    ///
    /// Whitespace is *not* collapsed here: shaping sees the string it is given,
    /// and CSS white-space processing belongs to layout. Shaping errors and font
    /// fallback are reported through [`Self::warnings`], never as a panic.
    #[must_use]
    pub fn shape(&mut self, text: &str, style: &TextStyle) -> ShapedText {
        let key = ShapeKey::new(text, style);
        if let Some(hit) = self.shaped.get(&key) {
            return hit.clone();
        }
        let shaped = self.shape_uncached(text, style);
        if self.shaped.len() >= MAX_SHAPED_CACHE {
            self.shaped.clear();
        }
        self.shaped.insert(key, shaped.clone());
        shaped
    }

    /// Number of shaped runs currently cached (diagnostics and tests).
    #[must_use]
    pub fn cached_shapes(&self) -> usize {
        self.shaped.len()
    }

    fn shape_uncached(&mut self, text: &str, style: &TextStyle) -> ShapedText {
        let size = style.effective_size();
        let Some(face_id) = self.face_for(style) else {
            return ShapedText::default();
        };
        let Some(face) = self.faces.get(&face_id) else {
            return ShapedText::default();
        };
        let (ascent, descent) = face.line_metrics(size);
        let mut shaped = ShapedText {
            glyphs: Vec::new(),
            width: 0.0,
            ascent,
            descent,
        };
        if text.is_empty() {
            return shaped;
        }

        let mut buffer = Buffer::new();
        buffer.push_str(text);
        // Picks direction and script from the first strong character; the
        // fallback for untyped text is left-to-right, which is what a browser
        // does for a paragraph without a `dir` attribute.
        buffer.guess_segment_properties();
        let shaper = ShaperFont::new(&face.font).with_scale(shaping_scale(size));
        if let Err(error) = harfrust::shape(&shaper, &mut buffer, ShapeOptions::new()) {
            push_warning(
                &mut self.warnings,
                format!("shaping `{text}` with `{}` failed: {error}", style.family),
            );
            return shaped;
        }

        let mut glyphs = Vec::with_capacity(buffer.len());
        let mut width = 0.0;
        for (info, position) in buffer
            .glyph_infos()
            .iter()
            .zip(buffer.glyph_positions().iter())
        {
            let advance = position.x_advance as f32 / SCALE_PER_PIXEL;
            width += advance;
            glyphs.push(ShapedGlyph {
                id: u16::try_from(info.glyph_id).unwrap_or(u16::MAX),
                x: position.x_offset as f32 / SCALE_PER_PIXEL,
                y: position.y_offset as f32 / SCALE_PER_PIXEL,
                advance,
            });
        }
        shaped.glyphs = glyphs;
        shaped.width = width;
        shaped
    }

    /// Rasterises one glyph of `style` into a coverage bitmap.
    ///
    /// `glyph_id` is a [`ShapedGlyph::id`] from [`Self::shape`] with the same
    /// style. Returns `None` when the face cannot draw that glyph — see
    /// [`crate::GlyphBitmap`] and the crate-level fallback table.
    #[must_use]
    pub fn rasterize(&mut self, glyph_id: u16, style: &TextStyle) -> Option<GlyphBitmap> {
        let size = style.effective_size();
        let face_id = self.face_for(style)?;
        let face = self.faces.get(&face_id)?;
        raster::rasterize(&mut self.raster_context, face, glyph_id, size)
    }

    /// PostScript name of `glyph_id` in the face selected for `style`.
    ///
    /// Diagnostic helper for tests, debugging and tooling; the renderer needs
    /// glyph ids, not names. Fonts without a `post` table get a synthesised
    /// `gidNNN` name from `read-fonts`.
    #[must_use]
    pub fn glyph_name(&mut self, glyph_id: u16, style: &TextStyle) -> Option<String> {
        let face_id = self.face_for(style)?;
        self.faces.get(&face_id)?.glyph_name(glyph_id)
    }

    /// Fallback diagnostics recorded so far, oldest first.
    ///
    /// Each entry is one human-readable sentence, deduplicated, so a missing
    /// family that is used on every line produces exactly one entry.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Resolves (and, on first use, loads) the face for `style`.
    ///
    /// Returns `None` only when the database has no usable font at all.
    fn face_for(&mut self, style: &TextStyle) -> Option<fontdb::ID> {
        let key = StyleKey::new(style);
        if let Some(cached) = self.resolutions.get(&key) {
            return *cached;
        }
        let Resolution { face, warning } =
            font::resolve(&self.database, &style.family, key.weight, key.italic);
        if let Some(message) = warning {
            push_warning(&mut self.warnings, message);
        }
        if let Some(id) = face {
            if !self.faces.contains_key(&id) {
                match LoadedFace::load(&self.database, id) {
                    Some(loaded) => {
                        self.faces.insert(id, loaded);
                    }
                    None => {
                        let name = self
                            .database
                            .face(id)
                            .and_then(|face| face.families.first())
                            .map(|(family, _)| family.clone())
                            .unwrap_or_else(|| "unknown".to_owned());
                        push_warning(
                            &mut self.warnings,
                            format!(
                                "font face `{name}` for family `{}` could not be loaded",
                                style.family
                            ),
                        );
                        self.resolutions.insert(key, None);
                        return None;
                    }
                }
            }
        }
        self.resolutions.insert(key, face);
        face
    }
}

impl Default for FontCache {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FontCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontCache")
            .field("loaded_faces", &self.faces.len())
            .field("resolved_styles", &self.resolutions.len())
            .field("warnings", &self.warnings)
            .finish_non_exhaustive()
    }
}

/// Shaping scale for `size` pixels, in 26.6 fixed point.
///
/// `size` has already been sanitised by `TextStyle::effective_size`, so the
/// product is inside `32..=65536` and the cast cannot saturate.
fn shaping_scale(size: f32) -> i32 {
    debug_assert!(size.is_finite() && size > 0.0, "size must be sanitised");
    (size * SCALE_PER_PIXEL).round() as i32
}

/// Records `message` unless an identical one is already present.
fn push_warning(warnings: &mut Vec<String>, message: String) {
    if !warnings.contains(&message) {
        warnings.push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shaped_runs_are_cached_by_text_and_style() {
        let mut fonts = FontCache::new();
        let style = TextStyle::default();
        let first = fonts.shape("Hello cache", &style);
        let cached = fonts.cached_shapes();
        assert!(cached >= 1);
        let second = fonts.shape("Hello cache", &style);
        assert_eq!(first, second);
        assert_eq!(fonts.cached_shapes(), cached);
        let other_style = TextStyle {
            size: 20.0,
            ..style
        };
        let _ = fonts.shape("Hello cache", &other_style);
        assert_eq!(fonts.cached_shapes(), cached + 1);
    }

    #[test]
    fn effective_size_defaults_degenerate_values() {
        for size in [f32::NAN, f32::INFINITY, 0.0, -4.0] {
            let style = TextStyle {
                size,
                ..TextStyle::default()
            };
            assert_eq!(style.effective_size(), DEFAULT_SIZE, "size {size}");
        }
    }

    #[test]
    fn effective_size_clamps_to_the_supported_range() {
        let too_small = TextStyle {
            size: 0.01,
            ..TextStyle::default()
        };
        assert_eq!(too_small.effective_size(), MIN_SIZE);
        let too_large = TextStyle {
            size: 1.0e9,
            ..TextStyle::default()
        };
        assert_eq!(too_large.effective_size(), MAX_SIZE);
        let exact = TextStyle {
            size: 13.5,
            ..TextStyle::default()
        };
        assert_eq!(exact.effective_size(), 13.5);
    }

    #[test]
    fn shaping_scale_is_26_6_fixed_point() {
        assert_eq!(shaping_scale(16.0), 1024);
        assert_eq!(shaping_scale(13.5), 864);
        assert_eq!(shaping_scale(MIN_SIZE), 32);
    }

    #[test]
    fn warnings_are_deduplicated() {
        let mut warnings = Vec::new();
        push_warning(&mut warnings, "family `Foo` is missing".to_owned());
        push_warning(&mut warnings, "family `Foo` is missing".to_owned());
        assert_eq!(warnings.len(), 1);
        push_warning(&mut warnings, "family `Bar` is missing".to_owned());
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn style_keys_normalise_the_family() {
        let key = StyleKey::new(&TextStyle::new("  Helvetica  ", 16.0, 400, false));
        assert_eq!(key.family, "helvetica");
        assert_eq!(key.weight, 400);
        assert!(!key.italic);
        let heavy = StyleKey::new(&TextStyle::new("Helvetica", 16.0, 5000, true));
        assert_eq!(heavy.weight, 1000, "weights are clamped for selection");
        assert!(heavy.italic);
    }
}
