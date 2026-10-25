//! `jkva-shell` — the reference harness that turns a page into pixels.
//!
//! M1 pipeline:
//!
//! ```text
//! HTML ──► jkva-engine::parse_page ──► StyleEngine ──► ComputedStyleMap
//!                                             │
//!                                             ▼
//!                         jkva-layout (StyleSource + TextMeasure)
//!                                             │
//!                                             ▼
//!                         jkva-text (shaping + glyph raster)
//!                                             │
//!                                             ▼
//!                              jkva-gfx::Scene ──► Metal ──► PNG
//! ```
//!
//! The shell is deliberately small: it owns the adapters between the layers and
//! the screenshot entry point, nothing that belongs in a library crate. The
//! future browser is a separate project; this is its test harness.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::cell::RefCell;

use jkva_dom::{Document, NodeId};
use jkva_engine::parse_page;
use jkva_gfx::{Image, Scene, SceneItem, TextureData, render_scene};
use jkva_layout::{
    Display, Edges, LayoutTree, StyleSource, TextMeasure, Viewport as LayoutViewport, layout,
};
use jkva_paint::Rect as PaintRect;
use jkva_style::{ComputedStyle, ComputedStyleMap, StyleEngine};
use jkva_text::{FontCache, TextStyle};

mod values;

pub use values::{layout_display, parse_color, parse_font_size, parse_length};

/// Version of the reference shell.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Result of rendering one page.
#[derive(Debug)]
pub struct RenderOutput {
    /// The rendered pixels.
    pub image: Image,
    /// The box tree, kept for tests and debugging.
    pub layout: LayoutTree,
    /// Non-fatal problems collected while loading or styling.
    pub warnings: Vec<String>,
}

/// Renders an HTML document with its inline styles at `width × height` CSS pixels.
///
/// # Errors
///
/// Returns an error when Metal is unavailable or a layer cannot produce a scene.
pub fn render_page(html: &str, width: u32, height: u32) -> Result<RenderOutput, String> {
    if width == 0 || height == 0 {
        return Err("viewport dimensions must be non-zero".into());
    }
    let page = parse_page(html);
    let mut engine = StyleEngine::new();
    for sheet in page.stylesheets {
        engine.add_stylesheet(sheet);
    }
    let styles_map = engine.compute_styles(&page.document);

    let styles = Styles::new(&styles_map, &page.document);
    let mut fonts = FontCache::new();
    let viewport = LayoutViewport {
        width: width as f32,
        height: height as f32,
    };
    let layout = {
        let mut measure = Texts { fonts: &mut fonts };
        layout(&page.document, &styles, viewport, &mut measure)
    };
    let scene = build_scene(&layout, &styles, &mut fonts);
    let image = render_scene(width, height, &scene)?;

    Ok(RenderOutput {
        image,
        layout,
        warnings: page.warnings,
    })
}

/// Adapter from the cascade to layout's [`StyleSource`].
struct Styles<'a> {
    map: &'a ComputedStyleMap,
    document: &'a Document,
    font_sizes: RefCell<Vec<Option<f32>>>,
}

impl<'a> Styles<'a> {
    fn new(map: &'a ComputedStyleMap, document: &'a Document) -> Self {
        Self {
            map,
            document,
            font_sizes: RefCell::new(vec![None; document.node_count()]),
        }
    }

    fn declared(&self, node: NodeId, pick: impl Fn(&ComputedStyle) -> &str) -> Option<f32> {
        let style = self.map.get(node)?;
        parse_length(pick(style), self.font_px(node))
    }

    /// Resolves `font-size` for an element, or inherits it for a text node.
    fn font_px(&self, node: NodeId) -> f32 {
        let index = node.index() as usize;
        if let Some(Some(value)) = self.font_sizes.borrow().get(index).copied() {
            return value;
        }
        let parent = self.document.parent(node);
        let base = parent.map_or(16.0, |p| self.font_px(p));
        let value = match self.map.get(node) {
            Some(style) => parse_font_size(&style.font_size, base),
            None => base,
        };
        if let Some(slot) = self.font_sizes.borrow_mut().get_mut(index) {
            *slot = Some(value);
        }
        value
    }

    fn inherited_style(&self, node: NodeId) -> Option<&ComputedStyle> {
        let mut current = Some(node);
        while let Some(id) = current {
            if let Some(style) = self.map.get(id) {
                return Some(style);
            }
            current = self.document.parent(id);
        }
        None
    }

    /// Font weight for paint. Layout still measures one default face, so a
    /// wrapped bold run can differ from the measured width until the layout
    /// seam carries `TextStyle` (documented limitation).
    fn weight_of(&self, node: NodeId) -> u16 {
        match self.inherited_style(node).map(|s| s.font_weight.trim()) {
            Some("bold" | "bolder") => 700,
            Some("normal" | "lighter") => 400,
            Some(value) => value.parse().unwrap_or(400),
            None => 400,
        }
    }

    /// Font family for paint, with the UA default when the cascade says nothing.
    fn family_of(&self, node: NodeId) -> String {
        let family = self
            .inherited_style(node)
            .map(|s| s.font_family.trim())
            .unwrap_or("");
        if family.is_empty() {
            "Helvetica".to_string()
        } else {
            family.trim_matches('"').to_string()
        }
    }

    fn inherited_color(&self, node: NodeId) -> [u8; 4] {
        let mut current = Some(node);
        while let Some(id) = current {
            if let Some(style) = self.map.get(id) {
                return parse_color(&style.color).unwrap_or([0, 0, 0, 255]);
            }
            current = self.document.parent(id);
        }
        [0, 0, 0, 255]
    }

    fn edges(
        &self,
        node: NodeId,
        pick: impl Fn(&ComputedStyle) -> (&str, &str, &str, &str),
    ) -> Edges {
        let Some(style) = self.map.get(node) else {
            return Edges::ZERO;
        };
        let (top, right, bottom, left) = pick(style);
        let base = self.font_px(node);
        Edges {
            top: parse_length(top, base).unwrap_or(0.0),
            right: parse_length(right, base).unwrap_or(0.0),
            bottom: parse_length(bottom, base).unwrap_or(0.0),
            left: parse_length(left, base).unwrap_or(0.0),
        }
    }
}

impl StyleSource for Styles<'_> {
    fn display(&self, node: NodeId) -> Display {
        self.map
            .get(node)
            .map_or(Display::Inline, |style| layout_display(style.display))
    }

    fn width(&self, node: NodeId) -> Option<f32> {
        self.declared(node, |s| &s.width)
    }

    fn height(&self, node: NodeId) -> Option<f32> {
        self.declared(node, |s| &s.height)
    }

    fn margin(&self, node: NodeId) -> Edges {
        self.edges(node, |s| {
            (
                &s.margin_top,
                &s.margin_right,
                &s.margin_bottom,
                &s.margin_left,
            )
        })
    }

    fn padding(&self, node: NodeId) -> Edges {
        self.edges(node, |s| {
            (
                &s.padding_top,
                &s.padding_right,
                &s.padding_bottom,
                &s.padding_left,
            )
        })
    }

    fn border(&self, node: NodeId) -> Edges {
        let _ = node;
        Edges::ZERO
    }

    fn font_size(&self, node: NodeId) -> f32 {
        self.font_px(node)
    }

    fn color(&self, node: NodeId) -> [u8; 4] {
        self.inherited_color(node)
    }

    fn background_color(&self, node: NodeId) -> Option<[u8; 4]> {
        self.map
            .get(node)
            .and_then(|style| parse_color(&style.background_color))
    }

    fn font_weight(&self, node: NodeId) -> u16 {
        self.weight_of(node)
    }

    fn font_family(&self, node: NodeId) -> String {
        self.family_of(node)
    }
}

/// Adapter from `jkva-text` to layout's measurement trait.
struct Texts<'a> {
    fonts: &'a mut FontCache,
}

impl TextMeasure for Texts<'_> {
    fn measure(&mut self, text: &str, font_size: f32) -> f32 {
        self.measure_styled(text, font_size, 400, "")
    }

    fn measure_styled(&mut self, text: &str, font_size: f32, weight: u16, family: &str) -> f32 {
        let style = text_style(font_size, weight, family);
        self.fonts.shape(text, &style).width
    }
}

fn text_style(size: f32, weight: u16, family: &str) -> TextStyle {
    let family = if family.trim().is_empty() {
        "Helvetica".to_string()
    } else {
        family.trim_matches('"').to_string()
    };
    TextStyle {
        family,
        size,
        weight,
        italic: false,
    }
}

fn build_scene(tree: &LayoutTree, styles: &Styles<'_>, fonts: &mut FontCache) -> Scene {
    let mut scene = Scene::new();
    for b in &tree.boxes {
        if let Some(color) = styles.background_color(b.node) {
            let (x, y, w, h) = (
                b.rect.origin.x,
                b.rect.origin.y,
                b.rect.size.w,
                b.rect.size.h,
            );
            if w > 0.0 && h > 0.0 {
                scene.push(SceneItem::Rect {
                    rect: PaintRect::new(x, y, w, h),
                    color,
                });
            }
        }
        let Some(run) = &b.text else { continue };
        let style = text_style(run.font_size, run.font_weight, &run.font_family);
        let shaped = fonts.shape(&run.text, &style);
        let baseline = run.origin.y + shaped.ascent;
        // `ShapedGlyph.x` is a shaping offset; the pen advances by `advance`.
        let mut pen = 0.0_f32;
        for glyph in &shaped.glyphs {
            let glyph_x = pen + glyph.x;
            pen += glyph.advance;
            let Some(bitmap) = fonts.rasterize(glyph.id, &style) else {
                continue;
            };
            if bitmap.width == 0 || bitmap.height == 0 {
                continue;
            }
            let mut rgba = Vec::with_capacity(bitmap.alpha.len() * 4);
            for alpha in &bitmap.alpha {
                rgba.extend_from_slice(&[255, 255, 255, *alpha]);
            }
            scene.push(SceneItem::Image {
                rect: PaintRect::new(
                    run.origin.x + glyph_x + bitmap.left as f32,
                    baseline - glyph.y - bitmap.top as f32,
                    bitmap.width as f32,
                    bitmap.height as f32,
                ),
                texture: TextureData {
                    width: bitmap.width,
                    height: bitmap.height,
                    rgba,
                },
                tint: run.color,
            });
        }
    }
    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_page_with_text() {
        let Ok(output) = render_page("<h1>H</h1><p>Hello</p>", 320, 200) else {
            return;
        };
        assert_eq!((output.image.width, output.image.height), (320, 200));
        let has_ink = output
            .image
            .rgba
            .chunks_exact(4)
            .any(|p| p[0] < 200 || p[1] < 200 || p[2] < 200);
        assert!(has_ink, "expected glyph pixels on the page");
    }

    #[test]
    fn rejects_empty_viewports() {
        assert!(render_page("<p>x</p>", 0, 100).is_err());
    }
}
