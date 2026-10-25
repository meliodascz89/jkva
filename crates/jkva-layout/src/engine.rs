//! The layout algorithm: block flow, inline flow, flex flow and greedy line breaking.
//!
//! # Algorithm
//!
//! One pass over the document, top-down, recursive. The pass owns a flat
//! `Vec<LayoutBox>`; the root box is reserved first so that index `0` is always the
//! document root, then every box is appended as it is placed.
//!
//! A block establishes a flow for its children. The children are flattened into
//! inline-level and block-level items (the internal `Flow` enum):
//!
//! * a text node becomes an inline item;
//! * an element with `display: inline` contributes its text to the inline flow and an
//!   inline box per line on top of it;
//! * an element with `display: block` becomes a block-level item;
//! * an element with `display: flex` becomes a block-level item that lays its children
//!   out with the flex algorithm;
//! * `display: none` removes the subtree.
//!
//! The flow cursor moves down the block's content box. Inline items accumulate tokens;
//! when a block-level item arrives (or the children end) the tokens are broken into
//! lines and placed, which is the anonymous-block behaviour CSS prescribes for mixed
//! content. Then the block child is placed at the cursor, its collapsed top margin
//! having been applied first.
//!
//! # Flex
//!
//! A flex container ([`Display::Flex`], a block-level box) lays its children out as a
//! single line of items. The modelled subset is `flex-direction: row | column`,
//! `justify-content` (five values), `align-items` (four values) and one `gap` for both
//! axes. Items are laid out in two steps: a *measurement pass* places every item at the
//! container's content origin with the width the algorithm gives it, then positions are
//! computed from the item sizes and each item's subtree is translated into place. There
//! is no second layout pass, because block content in this milestone does not depend on
//! its box height: `align-items: stretch` can therefore patch the used height of a row
//! item after its content has been laid out.
//!
//! Items are block-level children; an inline child is **blockified** (laid out as a
//! block item, which is what CSS does for flex items), and each bare text node child
//! becomes its own anonymous item. An item without a declared width is sized by a
//! documented max-content approximation: the widest line of its own inline content,
//! measured with [`TextMeasure`]; block-level descendants do not contribute to it.
//!
//! # Vertical margins
//!
//! Only adjacent block siblings collapse, and only as `max(margin_bottom, margin_top)`.
//! A whitespace-only text node between two blocks does not break the collapsing, because
//! it produces no line box; a line of text does. Negative margins are clamped to zero
//! before collapsing, so this implementation never needs the CSS rule that adds the
//! most negative margin. Parent/child collapsing, self-collapsing empty blocks and
//! clearance are not modelled — see the crate-level simplification list. Margins inside
//! a flex container never collapse.
//!
//! # Horizontal margins
//!
//! Horizontal margins never collapse. The border-box width of a block with
//! `width: auto` is the containing block's width minus its left and right margins;
//! explicit widths are content-box widths, plus padding and border.

use jkva_dom::{Document, NodeId, NodeKind};

use crate::geometry::{Point, Rect};
use crate::style::{
    AlignItems, Display, Edges, FlexDirection, JustifyContent, Length, StyleSource,
};
use crate::text::{LINE_HEIGHT_FACTOR, TextMeasure, TextRun};
use crate::tree::{LayoutBox, LayoutTree, Viewport};

/// Lays out `document` and returns the box tree.
///
/// This is the only entry point of the crate. It cannot fail: style values are
/// clamped (see [`StyleSource`]), a degenerate viewport lays out to an empty tree, a
/// `display: none` subtree is skipped, and text that does not fit is allowed to
/// overflow. The returned tree borrows nothing, so it can outlive the style source and
/// the text measurer.
///
/// # Panics
///
/// Does not panic for any document [`jkva_dom`] can build. Layout recurses over the
/// tree, so an adversarially deep document could exhaust the call stack; see the
/// crate-level simplification list.
///
/// # Example
///
/// ```
/// use jkva_dom::{Document, NodeId};
/// use jkva_layout::{Display, Edges, Length, LengthEdges, StyleSource, TextMeasure, Viewport, layout};
///
/// struct Blocky;
///
/// impl StyleSource for Blocky {
///     fn display(&self, _node: NodeId) -> Display { Display::Block }
///     fn width(&self, _node: NodeId) -> Length { Length::Auto }
///     fn height(&self, _node: NodeId) -> Length { Length::Auto }
///     fn margin(&self, _node: NodeId) -> LengthEdges { LengthEdges::ZERO }
///     fn padding(&self, _node: NodeId) -> LengthEdges { LengthEdges::ZERO }
///     fn border(&self, _node: NodeId) -> Edges { Edges::ZERO }
///     fn font_size(&self, _node: NodeId) -> f32 { 16.0 }
///     fn color(&self, _node: NodeId) -> [u8; 4] { [0, 0, 0, 255] }
///     fn background_color(&self, _node: NodeId) -> Option<[u8; 4]> { None }
/// }
///
/// struct TenPixels;
///
/// impl TextMeasure for TenPixels {
///     fn measure(&mut self, text: &str, _font_size: f32) -> f32 {
///         text.chars().count() as f32 * 10.0
///     }
/// }
///
/// let mut document = Document::new();
/// let body = document.create_element("body");
/// document.append_child(document.root(), body);
/// let text = document.create_text("Hello");
/// document.append_child(body, text);
///
/// let tree = layout(&document, &Blocky, Viewport::new(800.0, 600.0), &mut TenPixels);
/// assert_eq!(tree.len(), 3); // root, body, one text fragment
/// let run = tree.boxes[2].text.as_ref().expect("a text fragment");
/// assert_eq!(run.width, 50.0);
/// assert_eq!(run.origin, jkva_layout::Point::new(0.0, 0.0));
/// ```
#[must_use]
pub fn layout(
    document: &Document,
    styles: &dyn StyleSource,
    viewport: Viewport,
    text: &mut dyn TextMeasure,
) -> LayoutTree {
    Layouter {
        document,
        styles,
        text,
        boxes: Vec::new(),
    }
    .run(viewport)
}

/// The content box of a containing block: its left edge and its width.
///
/// The vertical edge is not part of this type because the flow cursor supplies it: a
/// containing block is horizontal geometry, and children are placed at a cursor the
/// parent owns.
#[derive(Clone, Copy, Debug)]
struct ContainingBlock {
    /// Left edge of the content box, in document coordinates.
    x: f32,
    /// Width of the content box in CSS pixels. Never negative.
    width: f32,
}

/// What one laid-out block contributes to its parent's flow.
#[derive(Clone, Copy, Debug)]
struct BlockMetrics {
    /// Index of the block's box in the tree under construction.
    index: usize,
    /// Border-box height, i.e. the distance the parent's cursor advances.
    border_height: f32,
}

/// A block box whose geometry the caller has already decided.
///
/// Block flow derives the geometry from the margins and the containing block, whereas
/// the flex algorithm decides the main and cross positions itself. Both end in
/// [`Layouter::layout_block_box`] through this struct, so a box is built exactly one
/// way.
struct BlockBox {
    /// Left edge of the border box.
    border_x: f32,
    /// Top edge of the border box.
    y: f32,
    /// Border-box width, i.e. content width plus padding and border.
    border_box_width: f32,
    /// Resolved padding.
    padding: Edges,
    /// Resolved border widths.
    border: Edges,
    /// Content height forced by the caller (`align-items: stretch`), if any.
    ///
    /// It wins over a declared height, and the item's content is allowed to overflow.
    forced_content_height: Option<f32>,
}

/// An item in the flow of one block, after inline boxes have been flattened.
#[derive(Clone, Debug)]
enum Flow {
    /// A text node with the inline elements it is nested in, outermost first.
    Text {
        /// The text node.
        node: NodeId,
        /// Inline ancestors whose boxes the text belongs to.
        inline: Vec<NodeId>,
    },
    /// A block-level element; it flushes the inline run and starts a block box.
    Block(NodeId),
}

/// One token of collapsed text.
///
/// Words and spaces are separated because line breaking needs a break opportunity,
/// while the fragment text needs the space in front of the word that follows a break.
#[derive(Clone, Debug)]
enum Token {
    /// A run of non-whitespace characters.
    Word {
        /// The characters, already collapsed.
        text: String,
        /// The text node the word came from.
        node: NodeId,
        /// Font size in CSS pixels.
        font_size: f32,
        /// Text colour as straight sRGB RGBA.
        color: [u8; 4],
        /// Font weight the word is measured at.
        weight: u16,
        /// Font family the word is measured at.
        family: String,
        /// Inline ancestors of the word, outermost first.
        inline: Vec<NodeId>,
    },
    /// One collapsible space. Consecutive spaces are merged into one.
    Space {
        /// The text node the space came from.
        node: NodeId,
        /// Font size the space is measured at.
        font_size: f32,
        /// Font weight the space is measured at.
        weight: u16,
        /// Font family the space is measured at.
        family: String,
        /// Text colour of the node the space came from.
        color: [u8; 4],
        /// Inline ancestors of the space, outermost first.
        inline: Vec<NodeId>,
    },
}

/// A text fragment waiting to be placed on a line.
#[derive(Clone, Debug)]
struct PendingFragment {
    /// The text node the fragment belongs to.
    node: NodeId,
    /// Fragment text, possibly starting with the space that continues a line.
    text: String,
    /// Additive advance of the fragment in CSS pixels.
    width: f32,
    /// Font size in CSS pixels.
    font_size: f32,
    /// Text colour as straight sRGB RGBA.
    color: [u8; 4],
    /// Font weight the fragment is measured at.
    weight: u16,
    /// Font family the fragment is measured at.
    family: String,
    /// Inline ancestors whose boxes this fragment contributes to.
    inline: Vec<NodeId>,
}

/// A line being filled with fragments.
#[derive(Debug, Default)]
struct Line {
    /// Fragments placed so far, in reading order.
    fragments: Vec<PendingFragment>,
    /// Sum of the fragment widths.
    width: f32,
    /// Largest font size on the line; drives the line box height.
    max_font_size: f32,
    /// A collapsible space seen since the last word, with its own style.
    pending_space: Option<PendingFragment>,
}

impl Line {
    /// Returns `true` when no fragment has been placed yet.
    fn is_empty(&self) -> bool {
        self.fragments.is_empty()
    }

    /// The line box height in CSS pixels.
    fn height(&self) -> f32 {
        LINE_HEIGHT_FACTOR * self.max_font_size
    }

    /// Appends a fragment, merging it into the previous one when they share a node.
    ///
    /// Merging yields one run per text node per line instead of one per word. A
    /// fragment that continues a line gets the separating space prepended to its text,
    /// so the space is part of the run that follows it; trailing spaces are therefore
    /// impossible and the line break does not leave them behind.
    fn push_fragment(&mut self, fragment: PendingFragment) {
        if let Some(last) = self.fragments.last_mut() {
            if last.node == fragment.node
                && last.font_size == fragment.font_size
                && last.color == fragment.color
                && last.weight == fragment.weight
                && last.family == fragment.family
            {
                last.text.push_str(&fragment.text);
                last.width += fragment.width;
                self.width += fragment.width;
                self.max_font_size = self.max_font_size.max(fragment.font_size);
                return;
            }
        }
        self.width += fragment.width;
        self.max_font_size = self.max_font_size.max(fragment.font_size);
        self.fragments.push(fragment);
    }
}

/// Mutable state of one block's flow.
struct FlowState {
    /// Where the next box is placed, in document coordinates.
    cursor: f32,
    /// Indices of the boxes emitted so far, in paint order.
    children: Vec<usize>,
    /// Bottom margin of the last block-level sibling, still waiting to collapse.
    ///
    /// It is kept as an `Option` because a line of text before the next block makes
    /// the margins non-adjacent: the line is an anonymous block box between them.
    pending_margin: Option<f32>,
}

/// A child of a flex container, before it has been measured.
#[derive(Clone, Copy, Debug)]
struct FlexNode {
    /// The node the item's box comes from: an element, or a text node for an
    /// anonymous item.
    node: NodeId,
    /// `true` when the item is the anonymous box wrapped around a bare text node.
    anonymous_text: bool,
}

/// One flex item after the measurement pass.
struct FlexItem {
    /// Used margins, resolved against the container's content width.
    margin: Edges,
    /// Resolved padding.
    padding: Edges,
    /// Resolved border widths.
    border: Edges,
    /// Content width the flex algorithm decided for the item.
    content_width: f32,
    /// Border-box height produced by the measurement layout (or by `stretch`).
    border_height: f32,
    /// Index of the item's own box, and so the first box of its subtree.
    start: usize,
    /// One past the last box of the item's subtree.
    end: usize,
    /// Whether the item declared a height; `align-items: stretch` must not override it.
    height_declared: bool,
    /// Content height forced by `stretch` once the line cross size is known.
    forced_height: Option<f32>,
    /// Effective cross-axis alignment: `align-self` or the container's `align-items`.
    align: AlignItems,
}

impl FlexItem {
    /// Outer (margin-box) width of the item.
    fn outer_width(&self) -> f32 {
        self.margin.horizontal()
            + self.padding.horizontal()
            + self.border.horizontal()
            + self.content_width
    }

    /// Outer (margin-box) height of the item.
    fn outer_height(&self) -> f32 {
        self.margin.vertical() + self.border_height
    }
}

/// Returns `(leading, between)` for distributing `free` space along the main axis.
///
/// `SpaceBetween` with a single item packs it at the start and `SpaceAround` centres it,
/// as CSS requires. The callers only pass non-negative free space, so an overflowing
/// line packs at the start for every variant (documented simplification).
fn justify_offsets(justify: JustifyContent, free: f32, count: usize) -> (f32, f32) {
    let count = count.max(1) as f32;
    match justify {
        JustifyContent::FlexStart => (0.0, 0.0),
        JustifyContent::FlexEnd => (free, 0.0),
        JustifyContent::Center => (free / 2.0, 0.0),
        JustifyContent::SpaceBetween => {
            if count > 1.0 {
                (0.0, free / (count - 1.0))
            } else {
                (0.0, 0.0)
            }
        }
        JustifyContent::SpaceAround => (free / (count * 2.0), free / count),
    }
}

/// Collapses two adjacent vertical margins.
///
/// The M1 model is "the larger of the two": negative margins are clamped to zero by
/// [`Edges::clamped`] before they reach here, and only adjacent siblings collapse. CSS
/// also adds the most negative margin when both signs are present and collapses a
/// parent's margins with its first and last child; neither is modelled yet.
fn collapse_margins(first: f32, second: f32) -> f32 {
    first.max(second)
}

/// Returns `rect` moved by `(dx, dy)`, keeping its extents exact.
///
/// The size is copied rather than recomputed through [`Rect::new`], so a sub-pixel
/// extent cannot drift when a flex item is translated.
fn translate_rect(rect: Rect, dx: f32, dy: f32) -> Rect {
    Rect {
        origin: Point::new(rect.origin.x + dx, rect.origin.y + dy),
        size: rect.size,
    }
}

/// Replaces negative and non-finite values with `0.0`.
///
/// `f32::max` returns the non-NaN operand, so a plain `max` would also work, but the
/// explicit check keeps infinities out of the box tree, and an infinite width would
/// make every later computation infinite.
fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

/// Returns `true` for the characters `white-space: normal` collapses.
///
/// Only the ASCII whitespace characters collapse. Unicode spaces such as U+00A0
/// NO-BREAK SPACE are ordinary, non-breaking characters: they stay inside the word, so
/// no line break can happen there. That is the CSS rule, and it is the one place where
/// `char::is_whitespace` cannot be used.
fn is_collapsible_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r' | '\u{000C}')
}

/// Appends a space token unless one is already pending or the run has not started.
///
/// Collapsing consecutive whitespace therefore happens at token level: the second,
/// third, … space of a run is dropped, and leading whitespace produces nothing.
fn push_space(
    tokens: &mut Vec<Token>,
    node: NodeId,
    font_size: f32,
    weight: u16,
    family: &str,
    color: [u8; 4],
    inline: &[NodeId],
) {
    if tokens.is_empty() || matches!(tokens.last(), Some(Token::Space { .. })) {
        return;
    }
    tokens.push(Token::Space {
        node,
        font_size,
        weight,
        family: family.to_string(),
        color,
        inline: inline.to_vec(),
    });
}

/// The layout pass: owns the output buffer and borrows its inputs.
struct Layouter<'a> {
    /// The document being laid out.
    document: &'a Document,
    /// The computed style the caller supplies.
    styles: &'a dyn StyleSource,
    /// The text measurement the caller supplies.
    text: &'a mut dyn TextMeasure,
    /// The boxes produced so far.
    boxes: Vec<LayoutBox>,
}

impl Layouter<'_> {
    /// Lays out the document root and returns the finished tree.
    fn run(mut self, viewport: Viewport) -> LayoutTree {
        let width = finite_non_negative(viewport.width);
        let viewport_height = finite_non_negative(viewport.height);
        let root_node = self.document.root();
        // Reserve index 0 before the children are laid out; the root's own height is
        // only known after them, so its box is patched at the end.
        self.boxes.push(LayoutBox {
            node: root_node,
            rect: Rect::EMPTY,
            content: Rect::EMPTY,
            children: Vec::new(),
            text: None,
        });
        let containing = ContainingBlock { x: 0.0, width };
        let (children, used_height) = self.layout_content(root_node, containing, 0.0);
        let root = LayoutBox {
            node: root_node,
            // The root covers at least the viewport, so a consumer can paint the page
            // background without checking the document height.
            rect: Rect::new(0.0, 0.0, width, used_height.max(viewport_height)),
            content: Rect::new(0.0, 0.0, width, used_height),
            children,
            text: None,
        };
        self.boxes[0] = root;
        LayoutTree {
            boxes: self.boxes,
            viewport,
        }
    }

    /// Lays out the children of a block or of the document root.
    ///
    /// Returns the child indices in paint order and the height the children used,
    /// measured from `top`.
    fn layout_content(
        &mut self,
        node: NodeId,
        containing: ContainingBlock,
        top: f32,
    ) -> (Vec<usize>, f32) {
        let flow = self.collect_flow(node);
        let mut state = FlowState {
            cursor: top,
            children: Vec::new(),
            pending_margin: None,
        };
        let mut tokens: Vec<Token> = Vec::new();
        for item in flow {
            match item {
                Flow::Text { node, inline } => self.collect_text(node, &inline, &mut tokens),
                Flow::Block(child) => {
                    // Text before a block becomes its own anonymous block, so the
                    // margins on either side of it are not adjacent any more.
                    self.flush_inline(&mut tokens, containing, &mut state);
                    let margin = self.used_margin(child, containing);
                    let gap = match state.pending_margin.take() {
                        Some(bottom) => collapse_margins(bottom, margin.top),
                        None => margin.top,
                    };
                    state.cursor += gap;
                    let metrics = self.layout_block(child, containing, state.cursor, margin);
                    state.children.push(metrics.index);
                    state.cursor += metrics.border_height;
                    state.pending_margin = Some(margin.bottom);
                }
            }
        }
        self.flush_inline(&mut tokens, containing, &mut state);
        if let Some(bottom) = state.pending_margin {
            state.cursor += bottom;
        }
        (state.children, state.cursor - top)
    }

    /// Resolves a block's used margins, including percentage and auto centring.
    fn used_margin(&self, node: NodeId, containing: ContainingBlock) -> Edges {
        let spec = self.styles.margin(node);
        let mut margin = spec.resolve(Some(containing.width)).clamped();
        let Some(width) = self.styles.width(node).resolve(Some(containing.width)) else {
            return margin;
        };
        let padding = self
            .styles
            .padding(node)
            .resolve(Some(containing.width))
            .clamped();
        let border = self.styles.border(node).clamped();
        let free = finite_non_negative(
            containing.width - width - padding.horizontal() - border.horizontal(),
        );
        let auto_left = spec.left == Length::Auto;
        let auto_right = spec.right == Length::Auto;
        if auto_left && auto_right {
            margin.left = free / 2.0;
            margin.right = free / 2.0;
        } else if auto_left {
            margin.left = free;
        } else if auto_right {
            margin.right = free;
        }
        margin
    }

    /// Lays out one block-level element in block flow.
    ///
    /// `y` is the top of the element's border box and `margin` its used margins; the
    /// caller has already applied the collapsed top margin to the flow cursor.
    fn layout_block(
        &mut self,
        node: NodeId,
        containing: ContainingBlock,
        y: f32,
        margin: Edges,
    ) -> BlockMetrics {
        let padding = self
            .styles
            .padding(node)
            .resolve(Some(containing.width))
            .clamped();
        let border = self.styles.border(node).clamped();
        let border_x = containing.x + margin.left;
        // An explicit width is a content-box width; `auto` fills the containing block
        // minus the horizontal margins.
        let border_box_width = match self.styles.width(node).resolve(Some(containing.width)) {
            Some(width) => width + padding.horizontal() + border.horizontal(),
            None => finite_non_negative(containing.width - margin.horizontal()),
        };
        self.layout_block_box(
            node,
            BlockBox {
                border_x,
                y,
                border_box_width,
                padding,
                border,
                forced_content_height: None,
            },
        )
    }

    /// Lays out one block box with caller-decided geometry.
    ///
    /// This is the single place that turns a [`BlockBox`] into a [`LayoutBox`]: it
    /// resolves the content box, runs the right formatting context for the children
    /// (block flow or flex), and resolves the used height. Block flow and the flex
    /// algorithm both call it, so an item and an in-flow block cannot disagree about
    /// padding, border or height resolution.
    fn layout_block_box(&mut self, node: NodeId, layout: BlockBox) -> BlockMetrics {
        let inner_width = finite_non_negative(
            layout.border_box_width - layout.padding.horizontal() - layout.border.horizontal(),
        );
        let content_x = layout.border_x + layout.border.left + layout.padding.left;
        let content_top = layout.y + layout.border.top + layout.padding.top;
        let index = self.boxes.len();
        self.boxes.push(LayoutBox {
            node,
            rect: Rect::EMPTY,
            content: Rect::EMPTY,
            children: Vec::new(),
            text: None,
        });
        let inner = ContainingBlock {
            x: content_x,
            width: inner_width,
        };
        let definite_height = self.styles.height(node).resolve(None);
        let (children, children_height) = if self.styles.display(node) == Display::Flex {
            self.layout_flex_content(node, inner, content_top, definite_height)
        } else {
            self.layout_content(node, inner, content_top)
        };
        let content_height = layout
            .forced_content_height
            .or(definite_height)
            .unwrap_or(children_height);
        let border_height = content_height + layout.padding.vertical() + layout.border.vertical();
        let rect = Rect::new(
            layout.border_x,
            layout.y,
            layout.border_box_width,
            border_height,
        );
        let content = Rect::new(content_x, content_top, inner_width, content_height);
        self.boxes[index] = LayoutBox {
            node,
            rect,
            content,
            children,
            text: None,
        };
        BlockMetrics {
            index,
            border_height,
        }
    }

    /// Lays out the children of a flex container and returns `(children, used height)`.
    ///
    /// One line, no wrapping: the main axis is horizontal for
    /// [`FlexDirection::Row`] and vertical for [`FlexDirection::Column`]. `definite_height`
    /// is the container's declared content height, which makes the cross size of a row
    /// container (and the main size of a column container) definite.
    ///
    /// Items are laid out once, at the container's content origin, and then translated
    /// into place. That is sound because block content in this milestone does not depend
    /// on its box height, so the measurement layout produces the same children wherever
    /// the item ends up.
    fn layout_flex_content(
        &mut self,
        node: NodeId,
        containing: ContainingBlock,
        top: f32,
        definite_height: Option<f32>,
    ) -> (Vec<usize>, f32) {
        let direction = self.styles.flex_direction(node);
        let justify = self.styles.justify_content(node);
        let align = self.styles.align_items(node);
        let gap = finite_non_negative(self.styles.gap(node));

        let mut nodes = Vec::new();
        self.collect_flex_items(node, &mut nodes);
        if nodes.is_empty() {
            return (Vec::new(), 0.0);
        }

        let mut items: Vec<FlexItem> = Vec::with_capacity(nodes.len());
        for flex_node in &nodes {
            let (margin, padding, border) = if flex_node.anonymous_text {
                // An anonymous item is a bare text run: it has no box decorations.
                (Edges::ZERO, Edges::ZERO, Edges::ZERO)
            } else {
                (
                    self.styles
                        .margin(flex_node.node)
                        .resolve(Some(containing.width))
                        .clamped(),
                    self.styles
                        .padding(flex_node.node)
                        .resolve(Some(containing.width))
                        .clamped(),
                    self.styles.border(flex_node.node).clamped(),
                )
            };
            let declared_width = if flex_node.anonymous_text {
                None
            } else {
                self.styles
                    .width(flex_node.node)
                    .resolve(Some(containing.width))
            };
            let declared_height = if flex_node.anonymous_text {
                None
            } else {
                self.styles.height(flex_node.node).resolve(None)
            };
            // A declared width wins. Otherwise a row item is sized to its max-content
            // approximation, and a column item stretches to the line unless the cross
            // axis is aligned instead of stretched.
            let item_align = self.styles.align_self(flex_node.node).unwrap_or(align);
            let content_width = match (direction, declared_width, item_align) {
                (_, Some(width), _) => width,
                (FlexDirection::Row, None, _) => {
                    self.max_content_width(flex_node.node, flex_node.anonymous_text)
                }
                (FlexDirection::Column, None, AlignItems::Stretch) => finite_non_negative(
                    containing.width
                        - margin.horizontal()
                        - padding.horizontal()
                        - border.horizontal(),
                ),
                (FlexDirection::Column, None, _) => {
                    self.max_content_width(flex_node.node, flex_node.anonymous_text)
                }
            };
            let start = self.boxes.len();
            let metrics = if flex_node.anonymous_text {
                self.layout_text_item(
                    flex_node.node,
                    ContainingBlock {
                        x: containing.x,
                        width: content_width,
                    },
                    top,
                )
            } else {
                self.layout_block_box(
                    flex_node.node,
                    BlockBox {
                        border_x: containing.x + margin.left,
                        y: top,
                        border_box_width: content_width
                            + padding.horizontal()
                            + border.horizontal(),
                        padding,
                        border,
                        forced_content_height: None,
                    },
                )
            };
            items.push(FlexItem {
                margin,
                padding,
                border,
                content_width,
                border_height: metrics.border_height,
                start,
                end: self.boxes.len(),
                height_declared: declared_height.is_some(),
                forced_height: None,
                align: item_align,
            });
        }

        // The line's cross size: a declared height makes it definite for a row, and a
        // column's cross axis is the container's content width, which is always definite.
        let line_cross = finite_non_negative(match direction {
            FlexDirection::Row => definite_height
                .unwrap_or_else(|| items.iter().map(FlexItem::outer_height).fold(0.0, f32::max)),
            FlexDirection::Column => containing.width,
        });

        // `stretch` gives an item without a declared height the line's cross size. The
        // block content has already been laid out from the item's content top, so
        // growing the box afterwards only moves its bottom edge.
        if direction == FlexDirection::Row {
            for item in &mut items {
                if item.height_declared || item.align != AlignItems::Stretch {
                    continue;
                }
                let target = finite_non_negative(
                    line_cross
                        - item.margin.vertical()
                        - item.padding.vertical()
                        - item.border.vertical(),
                );
                item.forced_height = Some(target);
                item.border_height = target + item.padding.vertical() + item.border.vertical();
            }
        }

        let sum_main: f32 = items
            .iter()
            .map(|item| match direction {
                FlexDirection::Row => item.outer_width(),
                FlexDirection::Column => item.outer_height(),
            })
            .sum();
        let gaps = gap * (items.len().saturating_sub(1)) as f32;
        let main_size = match direction {
            FlexDirection::Row => containing.width,
            // A column without a declared height is sized by its content, so there is
            // no free space for `justify-content` to distribute.
            FlexDirection::Column => definite_height.unwrap_or(sum_main + gaps),
        };
        let free = finite_non_negative(main_size - sum_main - gaps);
        let (leading, between) = justify_offsets(justify, free, items.len());

        let mut children = Vec::with_capacity(items.len());
        let mut cursor = match direction {
            FlexDirection::Row => containing.x,
            FlexDirection::Column => top,
        } + leading;
        for item in &items {
            if let Some(forced) = item.forced_height {
                let boxed = &mut self.boxes[item.start];
                let rect = boxed.rect;
                let content = boxed.content;
                boxed.rect = Rect::new(
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.w,
                    forced + item.padding.vertical() + item.border.vertical(),
                );
                boxed.content =
                    Rect::new(content.origin.x, content.origin.y, content.size.w, forced);
            }
            let outer_main = match direction {
                FlexDirection::Row => item.outer_width(),
                FlexDirection::Column => item.outer_height(),
            };
            let outer_cross = match direction {
                FlexDirection::Row => item.outer_height(),
                FlexDirection::Column => item.outer_width(),
            };
            let offset = match item.align {
                AlignItems::FlexStart | AlignItems::Stretch => 0.0,
                AlignItems::Center => (line_cross - outer_cross) / 2.0,
                AlignItems::FlexEnd => line_cross - outer_cross,
            };
            let main_start = cursor
                + match direction {
                    FlexDirection::Row => item.margin.left,
                    FlexDirection::Column => item.margin.top,
                };
            let cross_start = match direction {
                FlexDirection::Row => top,
                FlexDirection::Column => containing.x,
            } + offset
                + match direction {
                    FlexDirection::Row => item.margin.top,
                    FlexDirection::Column => item.margin.left,
                };
            let (planned_x, planned_y) = match direction {
                FlexDirection::Row => (main_start, cross_start),
                FlexDirection::Column => (cross_start, main_start),
            };
            self.translate_subtree(
                item.start..item.end,
                planned_x - (containing.x + item.margin.left),
                planned_y - top,
            );
            children.push(item.start);
            cursor += outer_main + gap + between;
        }

        let used_height = match direction {
            FlexDirection::Row => line_cross,
            FlexDirection::Column => definite_height.unwrap_or(sum_main + gaps),
        };
        (children, used_height)
    }

    /// Collects the flex items of a container, flattening document fragments.
    ///
    /// Every element that is not `display: none` is an item — including an inline
    /// element, which a flex container blockifies — and every bare text node becomes its
    /// own anonymous item. Comments are skipped, and so is whitespace-only text: CSS
    /// drops it, and HTML authors put it between elements on purpose; keeping it would
    /// add a zero-width item and an extra gap between the real ones.
    fn collect_flex_items(&self, node: NodeId, items: &mut Vec<FlexNode>) {
        for child in self.document.children(node) {
            self.collect_flex_item(child, items);
        }
    }

    /// Appends one child's contribution to the flex item list.
    fn collect_flex_item(&self, node: NodeId, items: &mut Vec<FlexNode>) {
        match self.document.kind(node) {
            NodeKind::Text(content) => {
                if content
                    .chars()
                    .any(|character| !is_collapsible_whitespace(character))
                {
                    items.push(FlexNode {
                        node,
                        anonymous_text: true,
                    });
                }
            }
            NodeKind::Comment(_) => {}
            // A detached document fragment is not reachable through `children`, but a
            // hand-built document can nest one; hoist its children as items.
            NodeKind::Document => {
                for child in self.document.children(node) {
                    self.collect_flex_item(child, items);
                }
            }
            NodeKind::Element(_) => {
                if self.styles.display(node) != Display::None {
                    items.push(FlexNode {
                        node,
                        anonymous_text: false,
                    });
                }
            }
        }
    }

    /// Approximates the max-content width of a flex item, in CSS pixels.
    ///
    /// The approximation is deliberately shallow: only the item's own inline content is
    /// measured, as if it were one line. Block-level descendants do not contribute, so
    /// an item whose content is only blocks has no intrinsic width and collapses to its
    /// padding and border. Real intrinsic sizing (and the `min-content`/`max-content`
    /// keywords) is out of scope for this increment.
    fn max_content_width(&mut self, node: NodeId, anonymous_text: bool) -> f32 {
        let mut tokens = Vec::new();
        if anonymous_text {
            self.collect_text(node, &[], &mut tokens);
        } else {
            for item in self.collect_flow(node) {
                if let Flow::Text {
                    node: text_node,
                    inline,
                } = item
                {
                    self.collect_text(text_node, &inline, &mut tokens);
                }
            }
        }
        let mut width = 0.0;
        let mut pending_space = 0.0;
        for token in &tokens {
            match token {
                Token::Word {
                    text,
                    font_size,
                    weight,
                    family,
                    ..
                } => {
                    width += pending_space
                        + finite_non_negative(
                            self.text.measure_styled(text, *font_size, *weight, family),
                        );
                    pending_space = 0.0;
                }
                Token::Space {
                    font_size,
                    weight,
                    family,
                    ..
                } => {
                    if width > 0.0 {
                        pending_space = finite_non_negative(
                            self.text.measure_styled(" ", *font_size, *weight, family),
                        );
                    }
                }
            }
        }
        width
    }

    /// Lays out a bare text node as an anonymous flex item.
    ///
    /// The item's box is the text node itself, so the box tree stays traceable back to
    /// the document; its children are the text fragments of the run.
    fn layout_text_item(
        &mut self,
        node: NodeId,
        containing: ContainingBlock,
        top: f32,
    ) -> BlockMetrics {
        let index = self.boxes.len();
        self.boxes.push(LayoutBox {
            node,
            rect: Rect::EMPTY,
            content: Rect::EMPTY,
            children: Vec::new(),
            text: None,
        });
        let mut state = FlowState {
            cursor: top,
            children: Vec::new(),
            pending_margin: None,
        };
        let mut tokens = Vec::new();
        self.collect_text(node, &[], &mut tokens);
        self.flush_inline(&mut tokens, containing, &mut state);
        let height = finite_non_negative(state.cursor - top);
        let rect = Rect::new(containing.x, top, containing.width, height);
        self.boxes[index] = LayoutBox {
            node,
            rect,
            content: rect,
            children: state.children,
            text: None,
        };
        BlockMetrics {
            index,
            border_height: height,
        }
    }

    /// Moves every box of one item's subtree by `(dx, dy)` in document space.
    ///
    /// The subtree is a contiguous range because layout appends a box and then
    /// everything below it; [`FlexItem::start`] and [`FlexItem::end`] bracket it.
    fn translate_subtree(&mut self, range: std::ops::Range<usize>, dx: f32, dy: f32) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        for boxed in &mut self.boxes[range] {
            boxed.rect = translate_rect(boxed.rect, dx, dy);
            boxed.content = translate_rect(boxed.content, dx, dy);
            if let Some(run) = &mut boxed.text {
                run.origin = Point::new(run.origin.x + dx, run.origin.y + dy);
            }
        }
    }

    /// Flattens the children of `node` into inline and block-level items.
    ///
    /// Inline elements are transparent at the flow level: their text is emitted in
    /// place, so it joins the surrounding run, and a block-level descendant is hoisted to
    /// the nearest block ancestor, which is what an anonymous-block split would do. The
    /// inline elements themselves become boxes per line, added in [`Self::flush_inline`].
    fn collect_flow(&self, node: NodeId) -> Vec<Flow> {
        let mut items = Vec::new();
        for child in self.document.children(node) {
            self.collect_flow_item(child, &[], &mut items);
        }
        items
    }

    /// Appends one node's contribution to `items`, recursing through inline boxes.
    fn collect_flow_item(&self, node: NodeId, inline: &[NodeId], items: &mut Vec<Flow>) {
        match self.document.kind(node) {
            NodeKind::Text(_) => items.push(Flow::Text {
                node,
                inline: inline.to_vec(),
            }),
            // Comments generate no box and are not even laid out.
            NodeKind::Comment(_) => {}
            // A detached document fragment is not reachable through `children`, but a
            // hand-built document can nest one; treat it as transparent.
            NodeKind::Document => {
                for child in self.document.children(node) {
                    self.collect_flow_item(child, inline, items);
                }
            }
            NodeKind::Element(_) => match self.styles.display(node) {
                Display::None => {}
                Display::Block | Display::Flex => items.push(Flow::Block(node)),
                Display::Inline => {
                    let mut nested = inline.to_vec();
                    nested.push(node);
                    for child in self.document.children(node) {
                        self.collect_flow_item(child, &nested, items);
                    }
                }
            },
        }
    }

    /// Collapses the whitespace of one text node into word and space tokens.
    fn collect_text(&self, node: NodeId, inline: &[NodeId], tokens: &mut Vec<Token>) {
        let NodeKind::Text(content) = self.document.kind(node) else {
            return;
        };
        let font_size = finite_non_negative(self.styles.font_size(node));
        let color = self.styles.color(node);
        let weight = self.styles.font_weight(node);
        let family = self.styles.font_family(node);
        let mut word = String::new();
        for character in content.chars() {
            if is_collapsible_whitespace(character) {
                if !word.is_empty() {
                    tokens.push(Token::Word {
                        text: std::mem::take(&mut word),
                        node,
                        font_size,
                        color,
                        weight,
                        family: family.clone(),
                        inline: inline.to_vec(),
                    });
                }
                push_space(tokens, node, font_size, weight, &family, color, inline);
            } else {
                word.push(character);
            }
        }
        if !word.is_empty() {
            tokens.push(Token::Word {
                text: word,
                node,
                font_size,
                color,
                weight,
                family,
                inline: inline.to_vec(),
            });
        }
    }

    /// Breaks the pending tokens into lines and emits one box per fragment.
    ///
    /// Does nothing when no line can be produced: no tokens, or whitespace only. The
    /// pending bottom margin is applied before the first line and only then, so a
    /// whitespace-only text node between two blocks does not defeat collapsing.
    fn flush_inline(
        &mut self,
        tokens: &mut Vec<Token>,
        containing: ContainingBlock,
        state: &mut FlowState,
    ) {
        if tokens.is_empty() {
            return;
        }
        let lines = self.break_lines(tokens, containing.width);
        if lines.is_empty() {
            return;
        }
        if let Some(bottom) = state.pending_margin.take() {
            state.cursor += bottom;
        }
        for line in lines {
            let line_height = line.height();
            let mut x = containing.x;
            let mut spans: Vec<(NodeId, f32, f32)> = Vec::new();
            for fragment in line.fragments {
                let left = x;
                let right = x + fragment.width;
                for &inline in &fragment.inline {
                    match spans.iter_mut().find(|(node, _, _)| *node == inline) {
                        Some((_, min, max)) => {
                            *min = min.min(left);
                            *max = max.max(right);
                        }
                        None => spans.push((inline, left, right)),
                    }
                }
                let rect = Rect::new(left, state.cursor, fragment.width, line_height);
                let index = self.boxes.len();
                self.boxes.push(LayoutBox {
                    node: fragment.node,
                    rect,
                    content: rect,
                    children: Vec::new(),
                    text: Some(TextRun {
                        text: fragment.text,
                        origin: Point::new(left, state.cursor),
                        font_size: fragment.font_size,
                        color: fragment.color,
                        font_weight: fragment.weight,
                        font_family: fragment.family,
                        width: fragment.width,
                    }),
                });
                state.children.push(index);
                x = right;
            }
            // One inline box per element per line, covering all of its fragments.
            for (node, left, right) in spans {
                let rect = Rect::new(left, state.cursor, right - left, line_height);
                let index = self.boxes.len();
                self.boxes.push(LayoutBox {
                    node,
                    rect,
                    content: rect,
                    children: Vec::new(),
                    text: None,
                });
                state.children.push(index);
            }
            state.cursor += line_height;
        }
    }

    /// Greedily fills lines from `tokens`, breaking at spaces.
    ///
    /// Measurement is additive: a line's width is the sum of its fragments. A word that
    /// does not fit on an empty line is placed anyway and overflows, because M1 has no
    /// `overflow-wrap`, no hyphenation and no `word-break`.
    fn break_lines(&mut self, tokens: &mut Vec<Token>, available: f32) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut line = Line::default();
        for token in tokens.drain(..) {
            match token {
                Token::Space {
                    node,
                    font_size,
                    weight,
                    family,
                    color,
                    inline,
                } => {
                    if !line.is_empty() {
                        let width = finite_non_negative(
                            self.text.measure_styled(" ", font_size, weight, &family),
                        );
                        line.pending_space = Some(PendingFragment {
                            node,
                            text: " ".to_string(),
                            width,
                            font_size,
                            color,
                            weight,
                            family,
                            inline,
                        });
                    }
                }
                Token::Word {
                    text,
                    node,
                    font_size,
                    color,
                    weight,
                    family,
                    inline,
                } => {
                    let word_width = finite_non_negative(
                        self.text.measure_styled(&text, font_size, weight, &family),
                    );
                    let space_width = line.pending_space.as_ref().map_or(0.0, |s| s.width);
                    let needed = line.width + space_width + word_width;
                    let must_break = !line.is_empty() && needed > available;
                    if must_break {
                        lines.push(std::mem::take(&mut line));
                    } else if let Some(space) = line.pending_space.take() {
                        line.push_fragment(space);
                    }
                    line.push_fragment(PendingFragment {
                        node,
                        text,
                        width: word_width,
                        font_size,
                        color,
                        weight,
                        family,
                        inline,
                    });
                }
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Length, LengthEdges};

    /// A style source with fixed values, used to exercise the pure helpers.
    struct FixedStyle {
        inline: Vec<NodeId>,
        hidden: Vec<NodeId>,
    }

    impl FixedStyle {
        fn new() -> Self {
            Self {
                inline: Vec::new(),
                hidden: Vec::new(),
            }
        }
    }

    impl StyleSource for FixedStyle {
        fn display(&self, node: NodeId) -> Display {
            if self.hidden.contains(&node) {
                Display::None
            } else if self.inline.contains(&node) {
                Display::Inline
            } else {
                Display::Block
            }
        }

        fn width(&self, _node: NodeId) -> Length {
            Length::Auto
        }

        fn height(&self, _node: NodeId) -> Length {
            Length::Auto
        }

        fn margin(&self, _node: NodeId) -> LengthEdges {
            LengthEdges::ZERO
        }

        fn padding(&self, _node: NodeId) -> LengthEdges {
            LengthEdges::ZERO
        }

        fn border(&self, _node: NodeId) -> Edges {
            Edges::ZERO
        }

        fn font_size(&self, _node: NodeId) -> f32 {
            16.0
        }

        fn color(&self, _node: NodeId) -> [u8; 4] {
            [0, 0, 0, 255]
        }

        fn background_color(&self, _node: NodeId) -> Option<[u8; 4]> {
            None
        }
    }

    /// A measurer that charges nothing; the tokenizer does not measure.
    struct NoMeasure;

    impl TextMeasure for NoMeasure {
        fn measure(&mut self, _text: &str, _font_size: f32) -> f32 {
            0.0
        }
    }

    /// Tokenizes one text node through the real `collect_text`.
    fn text_tokens(content: &str) -> Vec<Token> {
        let mut document = Document::new();
        let node = document.create_text(content);
        let styles = FixedStyle::new();
        let mut measure = NoMeasure;
        let layouter = Layouter {
            document: &document,
            styles: &styles,
            text: &mut measure,
            boxes: Vec::new(),
        };
        let mut tokens = Vec::new();
        layouter.collect_text(node, &[], &mut tokens);
        tokens
    }

    /// Extracts the word texts from a token stream.
    fn words(tokens: &[Token]) -> Vec<&str> {
        tokens
            .iter()
            .filter_map(|token| match token {
                Token::Word { text, .. } => Some(text.as_str()),
                Token::Space { .. } => None,
            })
            .collect()
    }

    #[test]
    fn collapse_margins_takes_the_maximum() {
        assert_eq!(collapse_margins(10.0, 20.0), 20.0);
        assert_eq!(collapse_margins(20.0, 10.0), 20.0);
        assert_eq!(collapse_margins(0.0, 0.0), 0.0);
        assert_eq!(collapse_margins(0.0, 30.0), 30.0);
        assert_eq!(collapse_margins(30.0, 0.0), 30.0);
        assert_eq!(collapse_margins(12.5, 12.5), 12.5);
    }

    #[test]
    fn justify_offsets_distribute_the_free_space() {
        // 500px free after three items.
        assert_eq!(
            justify_offsets(JustifyContent::FlexStart, 500.0, 3),
            (0.0, 0.0)
        );
        assert_eq!(
            justify_offsets(JustifyContent::FlexEnd, 500.0, 3),
            (500.0, 0.0)
        );
        assert_eq!(
            justify_offsets(JustifyContent::Center, 500.0, 3),
            (250.0, 0.0)
        );
        let (leading, between) = justify_offsets(JustifyContent::SpaceBetween, 500.0, 3);
        assert_eq!((leading, between), (0.0, 250.0));
        let (leading, between) = justify_offsets(JustifyContent::SpaceAround, 500.0, 3);
        assert!((leading - 500.0 / 6.0).abs() < 1.0e-5);
        assert!((between - 500.0 / 3.0).abs() < 1.0e-5);
    }

    #[test]
    fn justify_offsets_handle_a_single_item() {
        // `space-between` packs one item at the start, `space-around` centres it.
        assert_eq!(
            justify_offsets(JustifyContent::SpaceBetween, 100.0, 1),
            (0.0, 0.0)
        );
        assert_eq!(
            justify_offsets(JustifyContent::SpaceAround, 100.0, 1),
            (50.0, 100.0)
        );
    }

    #[test]
    fn translate_rect_preserves_the_extents() {
        let rect = Rect::new(1.0, 2.0, 3.5, 4.5);
        assert_eq!(
            translate_rect(rect, 10.0, 20.0),
            Rect::new(11.0, 22.0, 3.5, 4.5)
        );
        assert_eq!(translate_rect(rect, 0.0, 0.0), rect);
        assert_eq!(
            translate_rect(rect, -1.0, -2.0),
            Rect::new(0.0, 0.0, 3.5, 4.5)
        );
    }

    #[test]
    fn finite_non_negative_clamps_bad_numbers() {
        assert_eq!(finite_non_negative(5.0), 5.0);
        assert_eq!(finite_non_negative(0.0), 0.0);
        assert_eq!(finite_non_negative(-5.0), 0.0);
        assert_eq!(finite_non_negative(f32::NAN), 0.0);
        assert_eq!(finite_non_negative(f32::INFINITY), 0.0);
    }

    #[test]
    fn whitespace_collapses_to_single_spaces_without_edges() {
        let tokens = text_tokens("  a   b\n c  ");
        assert_eq!(words(&tokens), ["a", "b", "c"]);
        // word, space, word, space, word, space: no leading space, and the runs of
        // spaces collapse to one token each. The final space stays a token but can
        // never become a fragment, because a line never ends with one.
        assert_eq!(tokens.len(), 6);
        assert!(matches!(tokens[1], Token::Space { .. }));
        assert!(matches!(tokens[3], Token::Space { .. }));
        assert!(matches!(tokens[5], Token::Space { .. }));
    }

    #[test]
    fn whitespace_only_text_produces_no_word() {
        let tokens = text_tokens(" \t\r\n\u{000C} ");
        assert!(tokens.is_empty());
        assert!(words(&tokens).is_empty());
    }

    #[test]
    fn empty_text_produces_no_token() {
        assert!(text_tokens("").is_empty());
    }

    #[test]
    fn no_break_space_is_kept_inside_the_word() {
        let tokens = text_tokens("a\u{00A0}b c");
        assert_eq!(words(&tokens), ["a\u{00A0}b", "c"]);
        // Exactly one break opportunity, between the NBSP word and `c`.
        assert_eq!(tokens.len(), 3);
    }

    #[test]
    fn words_of_adjacent_text_nodes_can_be_separated_or_not() {
        let mut document = Document::new();
        let first = document.create_text("foo ");
        let second = document.create_text("bar");
        let third = document.create_text("baz");
        let styles = FixedStyle::new();
        let mut measure = NoMeasure;
        let layouter = Layouter {
            document: &document,
            styles: &styles,
            text: &mut measure,
            boxes: Vec::new(),
        };
        let mut tokens = Vec::new();
        layouter.collect_text(first, &[], &mut tokens);
        layouter.collect_text(second, &[], &mut tokens);
        assert_eq!(words(&tokens), ["foo", "bar"]);
        assert_eq!(tokens.len(), 3);

        let mut tokens = Vec::new();
        layouter.collect_text(third, &[], &mut tokens);
        layouter.collect_text(second, &[], &mut tokens);
        assert_eq!(words(&tokens), ["baz", "bar"]);
        // No space token at the junction: the two words are glued together.
        assert_eq!(tokens.len(), 2);
    }

    #[test]
    fn collect_flow_flattens_inline_elements_and_skips_hidden_subtrees() {
        let mut document = Document::new();
        let body = document.create_element("body");
        let span = document.create_element("span");
        let inline_text = document.create_text("inline");
        let hidden = document.create_element("div");
        let hidden_text = document.create_text("hidden");
        let block = document.create_element("p");
        document.append_child(document.root(), body);
        document.append_child(body, span);
        document.append_child(span, inline_text);
        document.append_child(body, hidden);
        document.append_child(hidden, hidden_text);
        document.append_child(body, block);

        let styles = FixedStyle {
            inline: vec![span],
            hidden: vec![hidden],
        };
        let mut measure = NoMeasure;
        let layouter = Layouter {
            document: &document,
            styles: &styles,
            text: &mut measure,
            boxes: Vec::new(),
        };
        let flow = layouter.collect_flow(body);
        assert_eq!(flow.len(), 2);
        assert!(matches!(flow[0], Flow::Text { node, .. } if node == inline_text));
        assert!(matches!(flow[1], Flow::Block(node) if node == block));
        // The hidden subtree contributes nothing at all.
        assert!(
            !flow
                .iter()
                .any(|item| matches!(item, Flow::Text { node, .. } if *node == hidden_text))
        );
    }
}
