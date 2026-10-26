//! `jkva-layout` — the box tree and the M1 layout algorithm of the JKva engine.
//!
//! This crate owns the step between the DOM and painting: it turns a
//! [`jkva_dom::Document`] plus resolved styles into a flat tree of positioned boxes.
//! Per ADR-016 the long-term plan is a hybrid (own block/inline/float/table, `taffy`
//! for flex and grid). Today the crate has its own block, inline and single-line flex
//! formatting contexts; floats, grid, tables, positioning and fragmentation do not
//! exist yet.
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
//! use jkva_layout::{Display, Edges, Length, LengthEdges, StyleSource, TextMeasure, Viewport, layout};
//!
//! struct Blocky;
//!
//! impl StyleSource for Blocky {
//!     fn display(&self, _node: NodeId) -> Display { Display::Block }
//!     fn width(&self, _node: NodeId) -> Length { Length::Auto }
//!     fn height(&self, _node: NodeId) -> Length { Length::Auto }
//!     fn margin(&self, _node: NodeId) -> LengthEdges { LengthEdges::ZERO }
//!     fn padding(&self, _node: NodeId) -> LengthEdges { LengthEdges::ZERO }
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
//! # What layout does
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
//!   (`1.2em`), and lines stack. An inline element gets one box per line covering its
//!   text on that line.
//! * **Flex flow.** An element with `display: flex` is a block-level flex container:
//!   `flex-direction: row | column`, `justify-content` (`flex-start`, `flex-end`,
//!   `center`, `space-between`, `space-around`), `align-items` (`stretch`,
//!   `flex-start`, `center`, `flex-end`), `align-self`, one `gap` for both axes,
//!   `order`, `align-content`, and `flex-wrap` / `flex-grow` / `flex-shrink` /
//!   `flex-basis` (see below). Its children
//!   are flex items; an inline child is blockified and each bare text node child becomes
//!   its own anonymous item. A row container breaks its items into lines when the next
//!   outer base size plus the gap would not fit, each line is aligned and justified on
//!   its own, and a line's free or missing space is distributed by the flex factors. A
//!   column container is always single-line. Items without a basis or declared width are
//!   sized by the maximum width of their own inline content (a documented approximation,
//!   see below), and `stretch` fills the line's cross size unless the item declares a
//!   height.
//! * **Grid flow.** An element with `display: grid` is a block-level grid container with a
//!   small `grid-template-columns` grammar (`Npx`, `Nfr`, `auto`, `N%`), implicit rows
//!   from `grid-auto-rows` (`Npx` or `auto`), one `gap` for both axes, and row-major
//!   placement. Items fill their cell unless they declare a size; there is no explicit
//!   placement, no spans, no dense packing and no `repeat()`/`minmax()`. `inline-grid` is
//!   not represented and falls back to inline flow today.
//! * **`display: none`.** The element and its whole subtree produce no boxes.
//!
//! # Simplifications (deliberate, documented)
//!
//! These are the known gaps of the current model. They are listed here so that a reader
//! can tell an unfinished feature from a bug.
//!
//! 1. **Flex wrapping and growing are row-only.** `flex-wrap`, `flex-grow` and
//!    `flex-shrink` are not applied to a column container, which stays single-line with
//!    its previous sizing; `flex-basis` is ignored there too, because a column's main
//!    axis is vertical and there is no vertical measure pass yet. `align-content` only
//!    applies to a row with several lines and a declared height; without a definite
//!    container height the lines keep their content heights and stack from the top.
//!    `order` is honoured for every flex container. `auto` margins on an item resolve to
//!    `0.0` instead of absorbing free space.
//! 2. **Flex sizes are approximations.** An item without a basis or declared width is as
//!    wide as the longest line of its *own inline content*; block descendants do not
//!    contribute, so an item whose content is only blocks collapses to its padding and
//!    border. Line breaking uses the outer base sizes, so growing or shrinking an item
//!    never moves it to another line. Shrinking is weighted by `flex-shrink × flex base
//!    size` and clamps at a zero content width without redistributing the remainder (CSS
//!    freezes such items and re-runs the distribution). There is no automatic minimum
//!    size, no `min-content`/`max-content` keyword, no `flex` shorthand and no
//!    text-based minimum width.
//! 3. **`inline-flex` is not a flex container.** [`Display`] has no inline-level flex
//!    variant; the engine maps `inline-flex` to [`Display::Inline`] today, so it is
//!    flattened into the surrounding inline flow. Mapping it to [`Display::Flex`] would
//!    make it behave as a block-level flex container, which is the intended fallback.
//! 4. **Grid is a first subset, and there is no float, table or `position`.** The grid
//!    track grammar is fixed (`Npx`, `Nfr`, `auto`, `N%`; no `repeat()`, `minmax()`, named
//!    lines or `fit-content()`), rows are always implicit (no `grid-template-rows`),
//!    placement is row-major (no explicit placement, spans, dense packing or `order` for
//!    grid items) and there is no `justify-items`/`align-items` for grid: an item fills its
//!    cell unless it declares a size, and a declared size leaves it start-aligned. `auto`
//!    tracks are sized by the max-content approximation of their widest item and are not
//!    stretched to fill the container. `inline-grid` is not represented. `float`,
//!    `table-*`, `list-item`, `inline-block` and `position` are still in normal flow and
//!    must be mapped to [`Display`] by the engine for now.
//! 5. **Margin collapsing is minimal.** Only adjacent block siblings collapse, as
//!    `max(bottom, top)`. Negative margins are clamped to zero instead of participating
//!    in the CSS rule that adds the most negative margin. Parent/child collapsing
//!    (the parent's top margin with the first child's, and the bottom margin with the
//!    last child's) is not modelled, so a child's vertical margins always stay inside
//!    the parent's content box. Self-collapsing empty blocks, clearance and tables of
//!    margins are absent. Margins inside a flex container never collapse.
//! 6. **Inline boxes are minimal.** An inline element's box covers its text per line and
//!    only the text style is applied: its margin, padding, border and background are not
//!    laid out yet, and a box wrapping across lines is one box per line with no
//!    continuity. A block-level descendant of an inline element is hoisted into the
//!    nearest block flow, in source order.
//! 7. **Text is not shaped here.** Layout measures through [`TextMeasure`], treats
//!    advances as additive, has no kerning across fragments, no hyphenation, no
//!    `word-break`/`overflow-wrap` (a word wider than a line overflows it), no
//!    `text-align`/`text-indent`/`letter-spacing`/`word-spacing`, and no bidirectional
//!    reordering. Unicode spaces other than the ASCII collapsible ones are kept inside
//!    words, so there is no break opportunity at a NO-BREAK SPACE.
//! 8. **Physical lengths only.** Absolute lengths and percentages of the containing
//!    block's width are supported for width, margin and padding; percentage heights
//!    behave like `auto`, because no containing block height is definite. `em`/`rem`,
//!    `vw`/`vh`, `min-*`/`max-*`, `box-sizing: border-box`, vertical writing modes and
//!    RTL page layout are absent (ADR-016 documents RTL text without RTL layout as a
//!    visible gap).
//! 9. **No overflow handling, no scrollbars, no scrolling.** An explicitly small box
//!    does not clip; a viewport smaller than the content does not shrink the content.
//! 10. **No fragmentation, no pagination, no `::first-line`.** Layout is one pass over
//!     one document; there is no incremental seam yet (ADR-016 wants one from M1, this
//!     pass is the whole-document baseline it will build on).
//! 11. **Recursive, not iterative.** Deep documents recurse; making the traversal
//!     iterative is required before layout can be fed hostile trees.
//! 12. **The root box is the page canvas.** It covers at least the viewport; its content
//!     box height is the used height of its children, so a consumer can tell the
//!     document height from it.
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
pub use style::{
    AlignContent, AlignItems, Display, Edges, FlexDirection, FlexWrap, GridAutoRows, GridTrack,
    JustifyContent, Length, LengthEdges, StyleSource, parse_grid_auto_rows,
    parse_grid_template_columns, parse_order,
};
pub use text::{LINE_HEIGHT_FACTOR, TextMeasure, TextRun};
pub use tree::{LayoutBox, LayoutTree, Viewport};

/// Version of the JKva layout crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
