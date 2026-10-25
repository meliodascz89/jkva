//! `jkva-layout` — the box tree and the M1 layout algorithm of the JKva engine.
//!
//! This crate owns the step between the DOM and painting: it turns a
//! [`jkva_dom::Document`] plus resolved styles into a flat tree of positioned boxes.
//! Per ADR-016 the long-term plan is a hybrid (own block/inline/float/table, `taffy`
//! for flex and grid); the flex, grid and table halves do not exist yet, and neither
//! do floats, positioning or fragmentation.
//!
//! # Pipeline
//!
//! ```text
//!   Document ─┐
//!   StyleSource ─┼─►  layout()  ─►  LayoutTree { boxes, viewport }
//!   Viewport ─┤                        │
//!   TextMeasure ─┘                     ▼
//!                                  jkva-paint (display lists)
//! ```
//!
//! Layout does **not** depend on `jkva-style` or `jkva-text`. Both are brought in
//! through traits the engine implements:
//!
//! * [`StyleSource`] answers "what is the display/width/margin/font of this node"
//!   over the cascade's computed style map;
//! * [`TextMeasure`] answers "how wide is this string at this font size" over shaping.
//!
//! The inversion keeps this crate small and its tests free of a font stack and a CSS
//! engine. Geometry types ([`Point`], [`Size`], [`Rect`]) are local copies for the same
//! reason; `jkva-paint` keeps its own, and a flat field copy moves a box between them.
//!
//! # Example
//!
//! ```
//! use jkva_dom::{Document, NodeId};
//! use jkva_layout::{Display, Edges, Length, StyleSource, TextMeasure, Viewport, layout};
//!
//! struct Blocky;
//!
//! impl StyleSource for Blocky {
//!     fn display(&self, _node: NodeId) -> Display { Display::Block }
//!     fn width(&self, _node: NodeId) -> Length { Length::Auto }
//!     fn height(&self, _node: NodeId) -> Length { Length::Auto }
//!     fn margin(&self, _node: NodeId) -> Edges { Edges::ZERO }
//!     fn padding(&self, _node: NodeId) -> Edges { Edges::ZERO }
//!     fn border(&self, _node: NodeId) -> Edges { Edges::ZERO }
//!     fn font_size(&self, _node: NodeId) -> f32 { 16.0 }
//!     fn color(&self, _node: NodeId) -> [u8; 4] { [0, 0, 0, 255] }
//!     fn background_color(&self, _node: NodeId) -> Option<[u8; 4]> { None }
//! }
//!
//! struct FixedAdvance;
//!
//! impl TextMeasure for FixedAdvance {
//!     fn measure(&mut self, text: &str, _font_size: f32) -> f32 {
//!         text.chars().count() as f32 * 10.0
//!     }
//! }
//!
//! let mut document = Document::new();
//! let body = document.create_element("body");
//! document.append_child(document.root(), body);
//! let text = document.create_text("Hello");
//! document.append_child(body, text);
//!
//! let tree = layout(&document, &Blocky, Viewport::new(800.0, 600.0), &mut FixedAdvance);
//! // One box for the document root, one for <body>, one text fragment.
//! assert_eq!(tree.len(), 3);
//! assert_eq!(tree.boxes[1].rect, tree.boxes[1].content);
//! ```
//!
//! # What M1 lays out
//!
//! * **Block flow.** Elements with `display: block` stack vertically in source order.
//!   `width: auto` fills the containing block minus the horizontal margins; an explicit
//!   width is a content-box width and is not constrained by its parent. `height: auto`
//!   is the children's height plus padding and border; an explicit height wins and
//!   content overflows.
//! * **Inline flow.** Text nodes (and the text inside inline elements) form line boxes
//!   in the nearest block. Consecutive collapsible whitespace becomes one space, edges
//!   of a line are trimmed, and lines break greedily at spaces when the next word would
//!   exceed the available width. Line height is `LINE_HEIGHT_FACTOR * font_size`
//!   (`1.2em`), and lines stack.
//! * **`display: none`.** The element and its whole subtree produce no boxes.
//!
//! # Simplifications (deliberate, documented)
//!
//! These are the known gaps of the M1 model. They are listed here so that a reader can
//! tell an unfinished feature from a bug.
//!
//! 1. **No float, no flex, no grid, no table, no `position`.** Every box is in normal
//!    flow. `inline-block`, `flex`, `grid`, `table-*` and `list-item` must be mapped to
//!    [`Display`] by the engine until M2.
//! 2. **Margin collapsing is minimal.** Only adjacent block siblings collapse, as
//!    `max(bottom, top)`. Negative margins are clamped to zero instead of participating
//!    in the CSS rule that adds the most negative margin. Parent/child collapsing
//!    (the parent's top margin with the first child's, and the bottom margin with the
//!    last child's) is not modelled, so a child's vertical margins always stay inside
//!    the parent's content box. Self-collapsing empty blocks, clearance and tables of
//!    margins are absent.
//! 3. **Inline elements are flattened.** An element with `display: inline` produces no
//!    box of its own: its text joins the parent block's inline flow, and its margin,
//!    padding, border, background, width and height are ignored. A block-level
//!    descendant of an inline element is hoisted into the nearest block flow, in source
//!    order. Splitting an inline box across lines (and thus inline backgrounds) is M2.
//! 4. **Text is not shaped here.** Layout measures through [`TextMeasure`], treats
//!    advances as additive, has no kerning across fragments, no hyphenation, no
//!    `word-break`/`overflow-wrap` (a word wider than a line overflows it), no
//!    `text-align`/`text-indent`/`letter-spacing`/`word-spacing`, and no bidirectional
//!    reordering. Unicode spaces other than the ASCII collapsible ones are kept inside
//!    words, so there is no break opportunity at a NO-BREAK SPACE.
//! 5. **Only physical, absolute lengths.** No `%`, `em`, `rem`, `vw`/`vh`, no
//!    `min-*`/`max-*`, no `box-sizing: border-box`, no vertical writing modes and no
//!    RTL page layout (ADR-016 documents RTL text without RTL layout as a visible gap).
//! 6. **No overflow handling, no scrollbars, no scrolling.** An explicitly small box
//!    does not clip; a viewport smaller than the content does not shrink the content.
//! 7. **No fragmentation, no pagination, no `::first-line`.** Layout is one pass over
//!    one document; there is no incremental seam yet (ADR-016 wants one from M1, this
//!    pass is the whole-document baseline it will build on).
//! 8. **Recursive, not iterative.** Deep documents recurse; making the traversal
//!    iterative is required before layout can be fed hostile trees.
//! 9. **The root box is the page canvas.** It covers at least the viewport; its content
//!    box height is the used height of its children, so a consumer can tell the
//!    document height from it.
//!
//! # Memory
//!
//! The tree is one `Vec<LayoutBox>` plus one `Vec<usize>` per box for children, so a
//! re-layout can reuse the allocation (ADR-008). Nothing is interned and no box holds a
//! reference into the document or into the style source.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod engine;
pub mod geometry;
pub mod style;
pub mod text;
pub mod tree;

pub use engine::layout;
pub use geometry::{Point, Rect, Size};
pub use style::{Display, Edges, Length, StyleSource};
pub use text::{LINE_HEIGHT_FACTOR, TextMeasure, TextRun};
pub use tree::{LayoutBox, LayoutTree, Viewport};

/// Version of the JKva layout crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
