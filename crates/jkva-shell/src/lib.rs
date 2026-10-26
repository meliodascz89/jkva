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
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use jkva_dom::{Document, NodeId, NodeKind};
use jkva_engine::parse_page;
use jkva_gfx::{Image, Scene, SceneItem, SceneRenderer, TextureData};
use jkva_layout::{
    AlignContent, AlignItems, Display, Edges, FlexDirection, FlexWrap, JustifyContent, LayoutTree,
    Length, LengthEdges, StyleSource, TextMeasure, Viewport as LayoutViewport, layout, parse_order,
};
use jkva_paint::{DamageRegion, Rect as PaintRect};
use jkva_style::{ComputedStyle, ComputedStyleMap, StyleEngine, Stylesheet};
use jkva_text::{FontCache, TextStyle};

mod values;

pub use values::{
    layout_display, parse_border_width, parse_color, parse_font_size, parse_length,
    parse_length_value,
};

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
    let mut frame = Frame::new(html, width, height)?;
    let image = frame.render()?;
    Ok(RenderOutput {
        image,
        layout: frame.layout,
        warnings: frame.warnings.clone(),
    })
}

/// A loaded, styled and laid-out document that can be mutated and re-measured.
///
/// This is the incremental seam: a mutation recomputes the style and layout for
/// the document, diffs the geometry against the previous tree and returns the
/// region that must be repainted. Matching and layout are still whole-document
/// passes; what is incremental today is the *damage*, which is the contract the
/// compositor will consume in M2/M3.
pub struct Frame {
    /// The document.
    pub document: Document,
    /// Author stylesheets found in the page.
    pub sheets: Vec<Stylesheet>,
    /// Computed styles for the current document.
    pub styles: ComputedStyleMap,
    /// The current box tree.
    pub layout: LayoutTree,
    /// Viewport the frame was measured at.
    pub viewport: LayoutViewport,
    /// Non-fatal problems collected while loading the page.
    pub warnings: Vec<String>,
    /// Damage from the most recent mutation, empty after [`Frame::new`].
    pub damage: DamageRegion,
    /// Rasterized glyphs reused across frames.
    glyphs: GlyphCache,
    /// Font database and face cache reused across frames.
    fonts: FontCache,
}

impl Frame {
    /// Loads, styles and lays out a page.
    ///
    /// # Errors
    ///
    /// Returns an error when the viewport has a zero dimension.
    pub fn new(html: &str, width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("viewport dimensions must be non-zero".into());
        }
        let page = parse_page(html);
        let viewport = LayoutViewport {
            width: width as f32,
            height: height as f32,
        };
        let styles = compute_styles(&page.document, &page.stylesheets);
        let mut fonts = FontCache::new();
        let layout = {
            let styles_adapter = Styles::new(&styles, &page.document);
            let mut measure = Texts { fonts: &mut fonts };
            layout(&page.document, &styles_adapter, viewport, &mut measure)
        };
        Ok(Self {
            document: page.document,
            sheets: page.stylesheets,
            styles,
            layout,
            viewport,
            warnings: page.warnings,
            damage: DamageRegion::new(),
            glyphs: GlyphCache::default(),
            fonts,
        })
    }

    /// Finds an element by its `id` attribute.
    #[must_use]
    pub fn element_by_id(&self, id: &str) -> Option<NodeId> {
        self.document
            .descendants(self.document.root())
            .find(|&node| {
                matches!(self.document.kind(node), NodeKind::Element(_))
                    && self
                        .document
                        .get_attribute(node, "id")
                        .is_some_and(|value| value == id)
            })
    }

    /// Sets an attribute, re-styles and re-lays out, and returns the damage.
    ///
    /// The mutated element is always damaged (its paint may change even when its
    /// geometry does not); boxes that moved, resized or changed text are added by
    /// the geometry diff.
    pub fn set_attribute(&mut self, node: NodeId, name: &str, value: &str) -> DamageRegion {
        let before = self.layout.boxes.clone();
        self.document.set_attribute(node, name, value);
        if let Some(old) = self.layout.box_for(node) {
            self.damage.add(paint_rect(old.rect));
        }
        self.restyle_and_relayout(&before);
        self.damage.clone()
    }

    fn restyle_and_relayout(&mut self, before: &[jkva_layout::LayoutBox]) {
        let sheets = self.sheets.clone();
        self.styles = compute_styles(&self.document, &sheets);
        let mut fonts = FontCache::new();
        let styles_adapter = Styles::new(&self.styles, &self.document);
        let mut measure = Texts { fonts: &mut fonts };
        self.layout = layout(&self.document, &styles_adapter, self.viewport, &mut measure);
        let diff = damage_between(before, &self.layout);
        self.damage.union(&diff);
    }

    /// Renders the current frame.
    ///
    /// # Errors
    ///
    /// Returns an error when Metal is unavailable or the scene cannot be built.
    pub fn render(&mut self) -> Result<Image, String> {
        let mut renderer = SceneRenderer::new()?;
        self.render_with(&mut renderer)
    }

    /// Renders the frame with a caller-owned renderer.
    ///
    /// This is the frame path: a game or an animation loop reuses one renderer
    /// across frames instead of creating a Metal device per screenshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the scene cannot be built or the GPU rejects the frame.
    pub fn render_with(&mut self, renderer: &mut SceneRenderer) -> Result<Image, String> {
        let styles_adapter = Styles::new(&self.styles, &self.document);
        let scene = build_scene(
            &self.layout,
            &styles_adapter,
            &mut self.fonts,
            &mut self.glyphs,
        );
        self.upload_glyphs(renderer)?;
        renderer.render(
            self.viewport.width as u32,
            self.viewport.height as u32,
            &scene,
        )
    }

    /// Makes every cached glyph texture known to the renderer (idempotent).
    fn upload_glyphs(&self, renderer: &mut SceneRenderer) -> Result<(), String> {
        for (key, glyph) in &self.glyphs.glyphs {
            renderer.ensure_texture(*key, &glyph.texture)?;
        }
        Ok(())
    }

    /// Draws the frame without reading the pixels back.
    ///
    /// # Errors
    ///
    /// Returns an error when the scene cannot be built or the GPU rejects the frame.
    pub fn submit_with(&mut self, renderer: &mut SceneRenderer) -> Result<(), String> {
        let styles_adapter = Styles::new(&self.styles, &self.document);
        let scene = build_scene(
            &self.layout,
            &styles_adapter,
            &mut self.fonts,
            &mut self.glyphs,
        );
        self.upload_glyphs(renderer)?;
        renderer.submit(
            self.viewport.width as u32,
            self.viewport.height as u32,
            &scene,
        )
    }
}

fn compute_styles(document: &Document, sheets: &[Stylesheet]) -> ComputedStyleMap {
    let mut engine = StyleEngine::new();
    for sheet in sheets {
        engine.add_stylesheet(sheet.clone());
    }
    engine.compute_styles(document)
}

fn paint_rect(rect: jkva_layout::Rect) -> PaintRect {
    PaintRect::new(rect.origin.x, rect.origin.y, rect.size.w, rect.size.h)
}

/// Union of the old and new geometry of every box whose geometry or text changed.
fn damage_between(before: &[jkva_layout::LayoutBox], after: &LayoutTree) -> DamageRegion {
    let mut damage = DamageRegion::new();
    let old: std::collections::HashMap<NodeId, &jkva_layout::LayoutBox> =
        before.iter().map(|b| (b.node, b)).collect();
    for new_box in &after.boxes {
        match old.get(&new_box.node) {
            Some(old_box) => {
                let moved = old_box.rect != new_box.rect
                    || old_box.content != new_box.content
                    || old_box.text != new_box.text;
                if moved {
                    damage.add(paint_rect(old_box.rect));
                    damage.add(paint_rect(new_box.rect));
                }
            }
            None => damage.add(paint_rect(new_box.rect)),
        }
    }
    damage
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

    fn declared_length(&self, node: NodeId, pick: impl Fn(&ComputedStyle) -> &str) -> Length {
        let Some(style) = self.map.get(node) else {
            return Length::Auto;
        };
        parse_length_value(pick(style), self.font_px(node))
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
    /// Resolves one border side's colour, falling back to `currentColor`.
    fn border_color(&self, node: NodeId, pick: impl Fn(&ComputedStyle) -> &str) -> [u8; 4] {
        let inherited = self.inherited_color(node);
        let Some(style) = self.map.get(node) else {
            return inherited;
        };
        let value = pick(style);
        if value.trim().eq_ignore_ascii_case("currentcolor") {
            inherited
        } else {
            parse_color(value).unwrap_or(inherited)
        }
    }

    fn number(&self, node: NodeId, pick: impl Fn(&ComputedStyle) -> &str) -> Option<f32> {
        let style = self.map.get(node)?;
        pick(style).trim().parse().ok()
    }

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
    ) -> LengthEdges {
        let Some(style) = self.map.get(node) else {
            return LengthEdges::ZERO;
        };
        let (top, right, bottom, left) = pick(style);
        let base = self.font_px(node);
        LengthEdges {
            top: parse_length_value(top, base),
            right: parse_length_value(right, base),
            bottom: parse_length_value(bottom, base),
            left: parse_length_value(left, base),
        }
    }
}

impl StyleSource for Styles<'_> {
    fn display(&self, node: NodeId) -> Display {
        self.map
            .get(node)
            .map_or(Display::Inline, |style| layout_display(style.display))
    }

    fn width(&self, node: NodeId) -> Length {
        self.declared_length(node, |s| &s.width)
    }

    fn height(&self, node: NodeId) -> Length {
        self.declared_length(node, |s| &s.height)
    }

    fn margin(&self, node: NodeId) -> LengthEdges {
        self.edges(node, |s| {
            (
                &s.margin_top,
                &s.margin_right,
                &s.margin_bottom,
                &s.margin_left,
            )
        })
    }

    fn padding(&self, node: NodeId) -> LengthEdges {
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
        let Some(style) = self.map.get(node) else {
            return Edges::ZERO;
        };
        let base = self.font_px(node);
        let width = |width: &str, style: &str| {
            if matches!(style.trim(), "none" | "hidden") {
                0.0
            } else {
                parse_border_width(width, base)
            }
        };
        Edges {
            top: width(&style.border_top_width, &style.border_top_style),
            right: width(&style.border_right_width, &style.border_right_style),
            bottom: width(&style.border_bottom_width, &style.border_bottom_style),
            left: width(&style.border_left_width, &style.border_left_style),
        }
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

    fn flex_direction(&self, node: NodeId) -> FlexDirection {
        match self.map.get(node).map(|s| s.flex_direction.as_str()) {
            Some("column") => FlexDirection::Column,
            _ => FlexDirection::Row,
        }
    }

    fn flex_wrap(&self, node: NodeId) -> FlexWrap {
        match self.map.get(node).map(|s| s.flex_wrap.as_str()) {
            Some("wrap") => FlexWrap::Wrap,
            _ => FlexWrap::NoWrap,
        }
    }

    fn flex_grow(&self, node: NodeId) -> f32 {
        self.number(node, |s| &s.flex_grow).unwrap_or(0.0)
    }

    fn flex_shrink(&self, node: NodeId) -> f32 {
        self.number(node, |s| &s.flex_shrink).unwrap_or(1.0)
    }

    fn flex_basis(&self, node: NodeId) -> Length {
        let Some(style) = self.map.get(node) else {
            return Length::Auto;
        };
        parse_length_value(&style.flex_basis, self.font_px(node))
    }

    fn justify_content(&self, node: NodeId) -> JustifyContent {
        match self.map.get(node).map(|s| s.justify_content.as_str()) {
            Some("flex-end") => JustifyContent::FlexEnd,
            Some("center") => JustifyContent::Center,
            Some("space-between") => JustifyContent::SpaceBetween,
            Some("space-around") => JustifyContent::SpaceAround,
            _ => JustifyContent::FlexStart,
        }
    }

    fn align_items(&self, node: NodeId) -> AlignItems {
        match self.map.get(node).map(|s| s.align_items.as_str()) {
            Some("flex-start") => AlignItems::FlexStart,
            Some("center") => AlignItems::Center,
            Some("flex-end") => AlignItems::FlexEnd,
            _ => AlignItems::Stretch,
        }
    }

    fn order(&self, node: NodeId) -> i32 {
        self.map
            .get(node)
            .map_or(0, |style| parse_order(&style.order))
    }

    fn align_content(&self, node: NodeId) -> AlignContent {
        match self.map.get(node).map(|s| s.align_content.as_str()) {
            Some("flex-start") => AlignContent::FlexStart,
            Some("flex-end") => AlignContent::FlexEnd,
            Some("center") => AlignContent::Center,
            Some("space-between") => AlignContent::SpaceBetween,
            Some("space-around") => AlignContent::SpaceAround,
            _ => AlignContent::Stretch,
        }
    }

    fn align_self(&self, node: NodeId) -> Option<AlignItems> {
        match self.map.get(node).map(|s| s.align_self.as_str()) {
            Some("flex-start") => Some(AlignItems::FlexStart),
            Some("center") => Some(AlignItems::Center),
            Some("flex-end") => Some(AlignItems::FlexEnd),
            Some("stretch") => Some(AlignItems::Stretch),
            _ => None,
        }
    }

    fn gap(&self, node: NodeId) -> f32 {
        self.map
            .get(node)
            .and_then(|s| parse_length(&s.gap, self.font_px(node)))
            .unwrap_or(0.0)
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

/// Rasterized glyphs keyed by glyph identity and style.
///
/// A glyph bitmap does not depend on its position, so the same glyph in the same
/// style can be uploaded once and reused for every frame that draws it.
#[derive(Debug, Default)]
struct GlyphCache {
    glyphs: HashMap<u64, CachedGlyph>,
}

/// One rasterized glyph bitmap plus its placement offsets.
#[derive(Debug, Clone)]
struct CachedGlyph {
    texture: TextureData,
    left: i32,
    top: i32,
}

/// Hashes the inputs that determine a glyph bitmap.
fn glyph_key(id: u16, style: &TextStyle) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    style.size.to_bits().hash(&mut hasher);
    style.weight.hash(&mut hasher);
    style.family.hash(&mut hasher);
    style.italic.hash(&mut hasher);
    hasher.finish()
}

fn build_scene(
    tree: &LayoutTree,
    styles: &Styles<'_>,
    fonts: &mut FontCache,
    glyphs: &mut GlyphCache,
) -> Scene {
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
        let border = styles.border(b.node);
        if border.top > 0.0 || border.right > 0.0 || border.bottom > 0.0 || border.left > 0.0 {
            let (x, y, w, h) = (
                b.rect.origin.x,
                b.rect.origin.y,
                b.rect.size.w,
                b.rect.size.h,
            );
            let top_color = styles.border_color(b.node, |s| &s.border_top_color);
            let right_color = styles.border_color(b.node, |s| &s.border_right_color);
            let bottom_color = styles.border_color(b.node, |s| &s.border_bottom_color);
            let left_color = styles.border_color(b.node, |s| &s.border_left_color);
            let mut push = |x: f32, y: f32, w: f32, h: f32, color: [u8; 4]| {
                if w > 0.0 && h > 0.0 {
                    scene.push(SceneItem::Rect {
                        rect: PaintRect::new(x, y, w, h),
                        color,
                    });
                }
            };
            push(x, y, w, border.top, top_color);
            push(x, y + h - border.bottom, w, border.bottom, bottom_color);
            push(x, y, border.left, h, left_color);
            push(x + w - border.right, y, border.right, h, right_color);
        }
    }

    // Glyphs are drawn after every background and border, so an inline
    // background sits behind the text it wraps instead of covering it.
    for b in &tree.boxes {
        let Some(run) = &b.text else { continue };
        let style = text_style(run.font_size, run.font_weight, &run.font_family);
        let shaped = fonts.shape(&run.text, &style);
        let baseline = run.origin.y + shaped.ascent;
        // `ShapedGlyph.x` is a shaping offset; the pen advances by `advance`.
        let mut pen = 0.0_f32;
        for glyph in &shaped.glyphs {
            let glyph_x = pen + glyph.x;
            pen += glyph.advance;
            let key = glyph_key(glyph.id, &style);
            let cached = match glyphs.glyphs.entry(key) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
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
                    entry.insert(CachedGlyph {
                        texture: TextureData {
                            width: bitmap.width,
                            height: bitmap.height,
                            rgba,
                        },
                        left: bitmap.left,
                        top: bitmap.top,
                    })
                }
            };
            scene.push(SceneItem::CachedImage {
                rect: PaintRect::new(
                    run.origin.x + glyph_x + cached.left as f32,
                    baseline - glyph.y - cached.top as f32,
                    cached.texture.width as f32,
                    cached.texture.height as f32,
                ),
                key,
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

    const DAMAGE_PAGE: &str = r#"
        <style>
        #a { background-color: red; }
        #a-alt { background-color: green; }
        #b { background-color: blue; margin-top: 200px; }
        .box { width: 100px; }
        .wide { width: 200px; }
        </style>
        <div id="a" class="box">A</div>
        <div id="b">B</div>
    "#;

    #[test]
    fn two_frames_with_one_renderer_are_identical() {
        let Ok(mut frame) = Frame::new("<p>Hello cache</p>", 200, 100) else {
            return;
        };
        let Ok(mut renderer) = SceneRenderer::new() else {
            return;
        };
        let first = frame.render_with(&mut renderer).unwrap();
        let second = frame.render_with(&mut renderer).unwrap();
        assert_eq!(first, second, "cached glyphs must not change the frame");
    }

    #[test]
    fn a_border_colour_is_painted() {
        let mut frame = Frame::new(
            "<style>#b { border: 2px solid #00ff00; width: 50px; height: 20px }</style><div id=\"b\"></div>",
            200,
            100,
        )
        .unwrap();
        let node = frame.element_by_id("b").unwrap();
        let rect = frame.layout.box_for(node).unwrap().rect;
        let image = frame.render().unwrap();
        let x = rect.origin.x as u32 + 1;
        let y = rect.origin.y as u32 + 1;
        let offset = ((y * image.width + x) * 4) as usize;
        assert_eq!(&image.rgba[offset..offset + 4], &[0, 255, 0, 255]);
    }

    #[test]
    fn an_inline_background_is_painted_behind_its_text() {
        let mut frame = Frame::new(
            "<style>#s { background-color: #ff0000 }</style><p>before <span id=\"s\">xx</span> after</p>",
            400,
            100,
        )
        .unwrap();
        let node = frame.element_by_id("s").unwrap();
        let rect = frame.layout.box_for(node).unwrap().rect;
        let image = frame.render().unwrap();
        let x = rect.origin.x as u32 + 1;
        let y = rect.origin.y as u32 + 1;
        let offset = ((y * image.width + x) * 4) as usize;
        assert_eq!(
            &image.rgba[offset..offset + 4],
            &[255, 0, 0, 255],
            "expected the inline background at ({x},{y})"
        );
    }

    #[test]
    fn percentage_margins_resolve_against_the_containing_width() {
        let frame = Frame::new(
            "<style>#m { margin-left: 10% }</style><div id=\"m\">x</div>",
            400,
            600,
        )
        .unwrap();
        let node = frame.element_by_id("m").unwrap();
        let rect = frame.layout.box_for(node).unwrap().rect;
        // Body content is 384px wide, so a 10% margin is 38.4px after the 8px body margin.
        assert!(
            (rect.origin.x - 46.4).abs() < 0.5,
            "expected x=46.4, got {}",
            rect.origin.x
        );
    }

    #[test]
    fn auto_margins_centre_a_fixed_width_box() {
        let frame = Frame::new(
            "<style>#c { width: 200px; margin: 0 auto }</style><div id=\"c\">x</div>",
            400,
            600,
        )
        .unwrap();
        let node = frame.element_by_id("c").unwrap();
        let rect = frame.layout.box_for(node).unwrap().rect;
        // (384 - 200) / 2 = 92, plus the 8px body margin.
        assert!(
            (rect.origin.x - 100.0).abs() < 0.5,
            "expected x=100, got {}",
            rect.origin.x
        );
        assert!((rect.width() - 200.0).abs() < 0.5);
    }

    #[test]
    fn percentage_width_resolves_against_the_containing_block() {
        let frame = Frame::new(
            "<style>#half { width: 50% }</style><div id=\"half\">x</div>",
            400,
            600,
        )
        .unwrap();
        let node = frame.element_by_id("half").unwrap();
        let rect = frame.layout.box_for(node).unwrap().rect;
        // The UA body margin is 8px on each side, so the containing width is 384.
        assert!(
            (rect.width() - 192.0).abs() < 0.5,
            "expected 192px, got {}",
            rect.width()
        );
    }

    #[test]
    fn a_fresh_frame_has_no_damage() {
        let frame = Frame::new(DAMAGE_PAGE, 400, 300).unwrap();
        assert!(frame.damage.is_empty());
    }

    #[test]
    fn a_background_change_damages_only_that_element() {
        let mut frame = Frame::new(DAMAGE_PAGE, 400, 300).unwrap();
        let a = frame.element_by_id("a").unwrap();
        let b = frame.element_by_id("b").unwrap();
        let a_rect = frame.layout.box_for(a).unwrap().rect;
        let b_rect = frame.layout.box_for(b).unwrap().rect;

        let damage = frame.set_attribute(a, "id", "a-alt");
        assert!(!damage.is_empty(), "a paint change must damage");
        let bounds = damage.bounds();
        let a_centre = jkva_paint::Point::new(a_rect.origin.x + 4.0, a_rect.origin.y + 4.0);
        let b_centre = jkva_paint::Point::new(b_rect.origin.x + 4.0, b_rect.origin.y + 4.0);
        assert!(
            bounds.contains(a_centre),
            "damage must cover the changed box"
        );
        assert!(
            !bounds.contains(b_centre),
            "damage must not cover a distant box"
        );
    }

    #[test]
    fn a_width_change_damages_the_old_and_new_geometry() {
        let mut frame = Frame::new(DAMAGE_PAGE, 400, 300).unwrap();
        let a = frame.element_by_id("a").unwrap();
        let damage = frame.set_attribute(a, "class", "wide");
        let bounds = damage.bounds();
        assert!(
            bounds.width() >= 199.0,
            "damage must span the old 100px and new 200px boxes, got {}",
            bounds.width()
        );
    }
}
