//! Glyph rasterisation through `swash`.
//!
//! One glyph in, one 8-bit coverage bitmap out; the caller decides where the
//! bitmap lives (the glyph atlas in `jkva-gfx` owns the reuse policy).

use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::Format;

use crate::GlyphBitmap;
use crate::font::LoadedFace;

/// Rasterises `glyph_id` at `size` CSS pixels per em.
///
/// The bitmap is in grayscale coverage (0 = fully transparent, 255 = fully
/// covered), positioned relative to the glyph's baseline origin: `left` is the
/// horizontal offset of the first column and `top` the vertical offset of the
/// first row, positive upwards.
///
/// Returns `None` when
///
/// - the face has no representation for `glyph_id` (out of range, missing
///   outline, bitmap-only face without strikes),
/// - the face produced an empty image, or
/// - the renderer produced something other than an alpha mask. Only outline
///   glyphs are requested today, so a colour (COLR/CPAL, `sbix`, CBDT) path is
///   not rendered yet — see the crate documentation.
///
/// Hinting is deliberately disabled: macOS renders unhinted, and `jkva-gfx`
/// rasterises at device resolution for Retina-first output (ADR-015).
pub(crate) fn rasterize(
    context: &mut ScaleContext,
    face: &LoadedFace,
    glyph_id: u16,
    size: f32,
) -> Option<GlyphBitmap> {
    let font = swash::FontRef::from_index(face.data.as_slice(), face.face_index as usize)?;
    let mut scaler = context.builder(font).size(size).hint(false).build();
    let sources = [Source::Outline];
    let mut render = Render::new(&sources);
    render.format(Format::Alpha);
    let image = render.render(&mut scaler, glyph_id)?;
    if image.content != Content::Mask {
        return None;
    }
    Some(GlyphBitmap {
        width: image.placement.width,
        height: image.placement.height,
        left: image.placement.left,
        top: image.placement.top,
        alpha: image.data,
    })
}
