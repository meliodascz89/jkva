//! The layout algorithm: block flow, inline flow and greedy line breaking.
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
//! * an element with `display: inline` contributes no box and its children are
//!   walked in its place, so the text participates in the surrounding inline flow;
//! * an element with `display: block` becomes a block-level item;
//! * `display: none` removes the subtree.
//!
//! The flow cursor moves down the block's content box. Inline items accumulate tokens;
//! when a block-level item arrives (or the children end) the tokens are broken into
//! lines and placed, which is the anonymous-block behaviour CSS prescribes for mixed
//! content. Then the block child is placed at the cursor, its collapsed top margin
//! having been applied first.
//!
//! # Vertical margins
//!
//! Only adjacent siblings collapse, and only as `max(margin_bottom, margin_top)`. A
//! whitespace-only text node between two blocks does not break the collapsing, because
//! it produces no line box; a line of text does. Negative margins are clamped to zero
//! before collapsing, so this implementation never needs the CSS rule that adds the
//! most negative margin. Parent/child collapsing, self-collapsing empty blocks and
//! clearance are not modelled — see the crate-level simplification list.
//!
//! # Horizontal margins
//!
//! Horizontal margins never collapse. The border-box width of a block with
//! `width: auto` is the containing block's width minus its left and right margins;
//! explicit widths are content-box widths, plus padding and border.

use jkva_dom::{Document, NodeId, NodeKind};

use crate::geometry::{Point, Rect};
use crate::style::{Display, Edges, StyleSource};
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
/// use jkva_layout::{Display, Edges, Length, StyleSource, TextMeasure, Viewport, layout};
///
/// struct Blocky;
///
/// impl StyleSource for Blocky {
///     fn display(&self, _node: NodeId) -> Display { Display::Block }
///     fn width(&self, _node: NodeId) -> Length { Length::Auto }
///     fn height(&self, _node: NodeId) -> Length { Length::Auto }
///     fn margin(&self, _node: NodeId) -> Edges { Edges::ZERO }
///     fn padding(&self, _node: NodeId) -> Edges { Edges::ZERO }
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

/// An item in the flow of one block, after inline boxes have been flattened.
#[derive(Clone, Copy, Debug)]
enum Flow {
    /// A text node; its words join the current inline run.
    Text(NodeId),
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

/// Collapses two adjacent vertical margins.
///
/// The M1 model is "the larger of the two": negative margins are clamped to zero by
/// [`Edges::clamped`] before they reach here, and only adjacent siblings collapse. CSS
/// also adds the most negative margin when both signs are present and collapses a
/// parent's margins with its first and last child; neither is modelled yet.
fn collapse_margins(first: f32, second: f32) -> f32 {
    first.max(second)
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
                Flow::Text(text_node) => self.collect_text(text_node, &mut tokens),
                Flow::Block(child) => {
                    // Text before a block becomes its own anonymous block, so the
                    // margins on either side of it are not adjacent any more.
                    self.flush_inline(&mut tokens, containing, &mut state);
                    let margin = self.styles.margin(child).clamped();
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

    /// Lays out one block-level element.
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
        let padding = self.styles.padding(node).clamped();
        let border = self.styles.border(node).clamped();
        let border_x = containing.x + margin.left;
        // An explicit width is a content-box width; `auto` fills the containing block
        // minus the horizontal margins.
        let border_box_width = match self.styles.width(node).resolve(Some(containing.width)) {
            Some(width) => width + padding.horizontal() + border.horizontal(),
            None => finite_non_negative(containing.width - margin.horizontal()),
        };
        let inner_width =
            finite_non_negative(border_box_width - padding.horizontal() - border.horizontal());
        let content_x = border_x + border.left + padding.left;
        let content_top = y + border.top + padding.top;
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
        let (children, children_height) = self.layout_content(node, inner, content_top);
        let content_height = match self.styles.height(node).resolve(None) {
            Some(height) if height.is_finite() && height >= 0.0 => height,
            _ => children_height,
        };
        let border_height = content_height + padding.vertical() + border.vertical();
        let rect = Rect::new(border_x, y, border_box_width, border_height);
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

    /// Flattens the children of `node` into inline and block-level items.
    ///
    /// Inline elements are transparent: their text is emitted in place, so it joins the
    /// surrounding run, and a block-level descendant is hoisted to the nearest block
    /// ancestor, which is what an anonymous-block split would do.
    fn collect_flow(&self, node: NodeId) -> Vec<Flow> {
        let mut items = Vec::new();
        for child in self.document.children(node) {
            self.collect_flow_item(child, &mut items);
        }
        items
    }

    /// Appends one node's contribution to `items`, recursing through inline boxes.
    fn collect_flow_item(&self, node: NodeId, items: &mut Vec<Flow>) {
        match self.document.kind(node) {
            NodeKind::Text(_) => items.push(Flow::Text(node)),
            // Comments generate no box and are not even laid out.
            NodeKind::Comment(_) => {}
            // A detached document fragment is not reachable through `children`, but a
            // hand-built document can nest one; treat it as transparent.
            NodeKind::Document => {
                for child in self.document.children(node) {
                    self.collect_flow_item(child, items);
                }
            }
            NodeKind::Element(_) => match self.styles.display(node) {
                Display::None => {}
                Display::Block => items.push(Flow::Block(node)),
                Display::Inline => {
                    for child in self.document.children(node) {
                        self.collect_flow_item(child, items);
                    }
                }
            },
        }
    }

    /// Collapses the whitespace of one text node into word and space tokens.
    fn collect_text(&self, node: NodeId, tokens: &mut Vec<Token>) {
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
                    });
                }
                push_space(tokens, node, font_size, weight, &family, color);
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
            for fragment in line.fragments {
                let rect = Rect::new(x, state.cursor, fragment.width, line_height);
                let index = self.boxes.len();
                self.boxes.push(LayoutBox {
                    node: fragment.node,
                    rect,
                    content: rect,
                    children: Vec::new(),
                    text: Some(TextRun {
                        text: fragment.text,
                        origin: Point::new(x, state.cursor),
                        font_size: fragment.font_size,
                        color: fragment.color,
                        font_weight: fragment.weight,
                        font_family: fragment.family,
                        width: fragment.width,
                    }),
                });
                state.children.push(index);
                x += fragment.width;
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
    use crate::style::Length;

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

        fn margin(&self, _node: NodeId) -> Edges {
            Edges::ZERO
        }

        fn padding(&self, _node: NodeId) -> Edges {
            Edges::ZERO
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
        layouter.collect_text(node, &mut tokens);
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
        layouter.collect_text(first, &mut tokens);
        layouter.collect_text(second, &mut tokens);
        assert_eq!(words(&tokens), ["foo", "bar"]);
        assert_eq!(tokens.len(), 3);

        let mut tokens = Vec::new();
        layouter.collect_text(third, &mut tokens);
        layouter.collect_text(second, &mut tokens);
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
        assert!(matches!(flow[0], Flow::Text(node) if node == inline_text));
        assert!(matches!(flow[1], Flow::Block(node) if node == block));
        // The hidden subtree contributes nothing at all.
        assert!(
            !flow
                .iter()
                .any(|item| matches!(item, Flow::Text(node) if *node == hidden_text))
        );
    }
}
