//! End-to-end tests of `jkva_layout::layout` against a mock style source and a
//! deterministic text measurer.
//!
//! Every expected number in this file is derivable by hand from the documented model:
//! the mocks charge half an em per character (`font_size * 0.5`), the default font size
//! is 16px, and a line box is `1.2 * font_size` tall. Tests that pin down a
//! simplification say so in a comment, so that a future change to the model has to
//! update them on purpose.

use std::collections::HashMap;

use jkva_dom::{Document, NodeId, parse_document};
use jkva_layout::{
    AlignContent, AlignItems, Display, Edges, FlexDirection, FlexWrap, GridAutoRows, GridTrack,
    JustifyContent, LayoutBox, LayoutTree, Length, LengthEdges, Rect, StyleSource, TextMeasure,
    TextRun, Viewport, layout,
};

const BLACK: [u8; 4] = [0, 0, 0, 255];

/// The style of one node as the mock reports it.
#[derive(Clone, Debug)]
struct NodeStyle {
    display: Display,
    width: Option<f32>,
    height: Option<f32>,
    margin: Edges,
    padding: Edges,
    border: Edges,
    font_size: f32,
    color: [u8; 4],
    background_color: Option<[u8; 4]>,
    flex_direction: FlexDirection,
    justify_content: JustifyContent,
    align_items: AlignItems,
    gap: f32,
    align_self: Option<AlignItems>,
    flex_wrap: FlexWrap,
    flex_grow: f32,
    flex_shrink: f32,
    flex_basis: Length,
    order: i32,
    align_content: AlignContent,
    grid_template_columns: Vec<GridTrack>,
    grid_auto_rows: GridAutoRows,
}

impl NodeStyle {
    /// A block with no box decorations at the default font size, in black.
    const BLOCK: Self = Self {
        display: Display::Block,
        width: None,
        height: None,
        margin: Edges::ZERO,
        padding: Edges::ZERO,
        border: Edges::ZERO,
        font_size: 16.0,
        color: BLACK,
        background_color: None,
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::FlexStart,
        align_items: AlignItems::Stretch,
        gap: 0.0,
        align_self: None,
        flex_wrap: FlexWrap::NoWrap,
        flex_grow: 0.0,
        flex_shrink: 1.0,
        flex_basis: Length::Auto,
        order: 0,
        align_content: AlignContent::Stretch,
        grid_template_columns: Vec::new(),
        grid_auto_rows: GridAutoRows::Auto,
    };

    /// An inline element: flattened by layout, so only text properties matter.
    fn inline() -> Self {
        Self {
            display: Display::Inline,
            ..Self::BLOCK
        }
    }

    /// A subtree that generates no boxes.
    fn none() -> Self {
        Self {
            display: Display::None,
            ..Self::BLOCK
        }
    }

    fn with_width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    fn with_height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    fn with_margin(mut self, margin: Edges) -> Self {
        self.margin = margin;
        self
    }

    fn with_padding(mut self, padding: Edges) -> Self {
        self.padding = padding;
        self
    }

    fn with_border(mut self, border: Edges) -> Self {
        self.border = border;
        self
    }

    fn with_align_self(mut self, align: AlignItems) -> Self {
        self.align_self = Some(align);
        self
    }

    fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    fn with_color(mut self, color: [u8; 4]) -> Self {
        self.color = color;
        self
    }

    /// A block-level flex container with items in a row.
    fn flex_row() -> Self {
        Self {
            display: Display::Flex,
            ..Self::BLOCK
        }
    }

    /// A block-level flex container with items in a column.
    fn flex_column() -> Self {
        Self {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            ..Self::BLOCK
        }
    }

    fn with_justify(mut self, justify: JustifyContent) -> Self {
        self.justify_content = justify;
        self
    }

    fn with_align(mut self, align: AlignItems) -> Self {
        self.align_items = align;
        self
    }

    fn with_gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    fn with_wrap(mut self) -> Self {
        self.flex_wrap = FlexWrap::Wrap;
        self
    }

    fn with_grow(mut self, grow: f32) -> Self {
        self.flex_grow = grow;
        self
    }

    fn with_shrink(mut self, shrink: f32) -> Self {
        self.flex_shrink = shrink;
        self
    }

    fn with_basis(mut self, basis: Length) -> Self {
        self.flex_basis = basis;
        self
    }

    fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    fn with_align_content(mut self, align_content: AlignContent) -> Self {
        self.align_content = align_content;
        self
    }

    /// A block-level grid container.
    fn grid() -> Self {
        Self {
            display: Display::Grid,
            ..Self::BLOCK
        }
    }

    fn with_columns(mut self, columns: Vec<GridTrack>) -> Self {
        self.grid_template_columns = columns;
        self
    }

    fn with_auto_rows(mut self, auto_rows: GridAutoRows) -> Self {
        self.grid_auto_rows = auto_rows;
        self
    }
}

/// A style source whose answers are a map, with a block default.
#[derive(Default)]
struct MockStyles {
    nodes: HashMap<NodeId, NodeStyle>,
}

impl MockStyles {
    fn new() -> Self {
        Self::default()
    }

    fn set(&mut self, node: NodeId, style: NodeStyle) {
        self.nodes.insert(node, style);
    }

    fn get(&self, node: NodeId) -> NodeStyle {
        self.nodes
            .get(&node)
            .cloned()
            .unwrap_or_else(|| NodeStyle::BLOCK.clone())
    }
}

impl StyleSource for MockStyles {
    fn display(&self, node: NodeId) -> Display {
        self.get(node).display
    }

    fn width(&self, node: NodeId) -> Length {
        self.get(node).width.map_or(Length::Auto, Length::px)
    }

    fn height(&self, node: NodeId) -> Length {
        self.get(node).height.map_or(Length::Auto, Length::px)
    }

    fn margin(&self, node: NodeId) -> LengthEdges {
        LengthEdges::from_edges(self.get(node).margin)
    }

    fn padding(&self, node: NodeId) -> LengthEdges {
        LengthEdges::from_edges(self.get(node).padding)
    }

    fn border(&self, node: NodeId) -> Edges {
        self.get(node).border
    }

    fn font_size(&self, node: NodeId) -> f32 {
        self.get(node).font_size
    }

    fn color(&self, node: NodeId) -> [u8; 4] {
        self.get(node).color
    }

    fn background_color(&self, node: NodeId) -> Option<[u8; 4]> {
        self.get(node).background_color
    }

    fn flex_direction(&self, node: NodeId) -> FlexDirection {
        self.get(node).flex_direction
    }

    fn justify_content(&self, node: NodeId) -> JustifyContent {
        self.get(node).justify_content
    }

    fn align_items(&self, node: NodeId) -> AlignItems {
        self.get(node).align_items
    }

    fn gap(&self, node: NodeId) -> f32 {
        self.get(node).gap
    }

    fn align_self(&self, node: NodeId) -> Option<AlignItems> {
        self.get(node).align_self
    }

    fn flex_wrap(&self, node: NodeId) -> FlexWrap {
        self.get(node).flex_wrap
    }

    fn flex_grow(&self, node: NodeId) -> f32 {
        self.get(node).flex_grow
    }

    fn flex_shrink(&self, node: NodeId) -> f32 {
        self.get(node).flex_shrink
    }

    fn flex_basis(&self, node: NodeId) -> Length {
        self.get(node).flex_basis
    }

    fn order(&self, node: NodeId) -> i32 {
        self.get(node).order
    }

    fn align_content(&self, node: NodeId) -> AlignContent {
        self.get(node).align_content
    }

    fn grid_template_columns(&self, node: NodeId) -> Vec<GridTrack> {
        self.get(node).grid_template_columns.clone()
    }

    fn grid_auto_rows(&self, node: NodeId) -> GridAutoRows {
        self.get(node).grid_auto_rows
    }
}

/// Charges half an em per character, so 16px text advances 8px per character.
struct HalfEm;

impl TextMeasure for HalfEm {
    fn measure(&mut self, text: &str, font_size: f32) -> f32 {
        text.chars().count() as f32 * font_size * 0.5
    }
}

fn layout_with(document: &Document, styles: &MockStyles, width: f32, height: f32) -> LayoutTree {
    layout(document, styles, Viewport::new(width, height), &mut HalfEm)
}

fn body_element(document: &mut Document) -> NodeId {
    let root = document.root();
    element(document, root, "body")
}

fn element(document: &mut Document, parent: NodeId, name: &str) -> NodeId {
    let node = document.create_element(name);
    document.append_child(parent, node);
    node
}

fn text(document: &mut Document, parent: NodeId, content: &str) -> NodeId {
    let node = document.create_text(content);
    document.append_child(parent, node);
    node
}

fn box_of(tree: &LayoutTree, node: NodeId) -> &LayoutBox {
    tree.box_for(node).expect("a box for the node")
}

/// The `x` positions of the boxes for `nodes`, in the given order.
fn xs(tree: &LayoutTree, nodes: &[NodeId]) -> Vec<f32> {
    nodes
        .iter()
        .map(|node| box_of(tree, *node).rect.origin.x)
        .collect()
}

/// The `y` positions of the boxes for `nodes`, in the given order.
fn ys(tree: &LayoutTree, nodes: &[NodeId]) -> Vec<f32> {
    nodes
        .iter()
        .map(|node| box_of(tree, *node).rect.origin.y)
        .collect()
}

/// Asserts a list of positions with a small tolerance.
fn assert_positions(got: Vec<f32>, want: &[f32]) {
    assert_eq!(got.len(), want.len(), "position count");
    for (index, (got, want)) in got.iter().zip(want).enumerate() {
        assert!(
            (got - want).abs() < 1.0e-4,
            "position {index}: expected {want}, got {got}"
        );
    }
}

fn text_runs(tree: &LayoutTree) -> Vec<&TextRun> {
    tree.boxes.iter().filter_map(|b| b.text.as_ref()).collect()
}

fn assert_close(got: f32, want: f32) {
    assert!((got - want).abs() < 1.0e-4, "expected {want}, got {got}");
}

#[test]
fn root_box_covers_at_least_the_viewport() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    text(&mut document, body, "Hi");
    let tree = layout_with(&document, &MockStyles::new(), 800.0, 600.0);

    // Root, <body>, one text fragment.
    assert_eq!(tree.len(), 3);
    let root = tree.root().expect("a root box");
    assert_eq!(root.node, document.root());
    assert_eq!(root.rect, Rect::new(0.0, 0.0, 800.0, 600.0));
    // The content box reports the used document height, not the viewport height.
    assert_close(root.content.height(), 19.2);
    assert_eq!(tree.viewport, Viewport::new(800.0, 600.0));
}

#[test]
fn empty_document_lays_out_just_the_root() {
    let document = Document::new();
    let tree = layout_with(&document, &MockStyles::new(), 640.0, 480.0);
    assert_eq!(tree.len(), 1);
    let root = tree.root().expect("a root box");
    assert_eq!(root.rect, Rect::new(0.0, 0.0, 640.0, 480.0));
    assert_eq!(root.content.height(), 0.0);
    assert!(root.children.is_empty());
}

#[test]
fn auto_width_fills_the_containing_block_minus_horizontal_margins() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let child = element(&mut document, body, "div");
    let mut styles = MockStyles::new();
    styles.set(
        child,
        NodeStyle::BLOCK
            .with_height(10.0)
            .with_margin(Edges::new(0.0, 30.0, 0.0, 10.0)),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let child_box = box_of(&tree, child);
    assert_eq!(child_box.rect, Rect::new(10.0, 0.0, 760.0, 10.0));
    // The body's auto height is the child's height.
    assert_close(box_of(&tree, body).content.height(), 10.0);
}

#[test]
fn explicit_width_is_a_content_box_width() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let child = element(&mut document, body, "div");
    let mut styles = MockStyles::new();
    styles.set(
        child,
        NodeStyle::BLOCK
            .with_width(100.0)
            .with_padding(Edges::all(10.0))
            .with_border(Edges::all(5.0)),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let child_box = box_of(&tree, child);
    // 100 content + 2*10 padding + 2*5 border.
    assert_close(child_box.rect.width(), 130.0);
    assert_close(child_box.rect.height(), 30.0);
    assert_close(child_box.content.width(), 100.0);
    assert_eq!(
        child_box.content.origin,
        jkva_layout::Point::new(15.0, 15.0)
    );
    // The box is narrower than its containing block, so nothing is stretched.
    assert_eq!(child_box.rect.origin, jkva_layout::Point::new(0.0, 0.0));
}

#[test]
fn explicit_width_larger_than_the_containing_block_overflows() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let child = element(&mut document, body, "div");
    let mut styles = MockStyles::new();
    styles.set(child, NodeStyle::BLOCK.with_width(900.0).with_height(5.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    assert_close(box_of(&tree, child).rect.width(), 900.0);
}

#[test]
fn blocks_stack_vertically_in_source_order() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let first = element(&mut document, body, "div");
    let second = element(&mut document, body, "div");
    let third = element(&mut document, body, "div");
    let mut styles = MockStyles::new();
    styles.set(first, NodeStyle::BLOCK.with_height(10.0));
    styles.set(second, NodeStyle::BLOCK.with_height(20.0));
    styles.set(third, NodeStyle::BLOCK.with_height(30.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    assert_close(box_of(&tree, first).rect.origin.y, 0.0);
    assert_close(box_of(&tree, second).rect.origin.y, 10.0);
    assert_close(box_of(&tree, third).rect.origin.y, 30.0);
    assert_close(box_of(&tree, body).content.height(), 60.0);
}

#[test]
fn adjacent_sibling_margins_collapse_to_the_larger_value() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let first = element(&mut document, body, "div");
    let second = element(&mut document, body, "div");
    let mut styles = MockStyles::new();
    styles.set(
        first,
        NodeStyle::BLOCK
            .with_height(10.0)
            .with_margin(Edges::new(0.0, 0.0, 20.0, 0.0)),
    );
    styles.set(
        second,
        NodeStyle::BLOCK
            .with_height(20.0)
            .with_margin(Edges::new(30.0, 0.0, 0.0, 0.0)),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    // max(20, 30) = 30, so the second block starts at 10 + 30, not at 10 + 50.
    assert_close(box_of(&tree, second).rect.origin.y, 40.0);
    assert_close(box_of(&tree, body).content.height(), 60.0);
}

#[test]
fn negative_margins_are_clamped_to_zero() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let first = element(&mut document, body, "div");
    let second = element(&mut document, body, "div");
    let mut styles = MockStyles::new();
    styles.set(
        first,
        NodeStyle::BLOCK
            .with_height(10.0)
            .with_margin(Edges::new(0.0, 0.0, -10.0, 0.0)),
    );
    styles.set(
        second,
        NodeStyle::BLOCK
            .with_height(20.0)
            .with_margin(Edges::new(20.0, 0.0, 0.0, 0.0)),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    // The negative bottom margin collapses to 0, so max(0, 20) = 20.
    assert_close(box_of(&tree, second).rect.origin.y, 30.0);
}

#[test]
fn a_line_of_text_makes_sibling_margins_non_adjacent() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let first = element(&mut document, body, "div");
    let between = text(&mut document, body, "hi");
    let second = element(&mut document, body, "div");
    let mut styles = MockStyles::new();
    styles.set(
        first,
        NodeStyle::BLOCK
            .with_height(10.0)
            .with_margin(Edges::new(0.0, 0.0, 20.0, 0.0)),
    );
    styles.set(
        second,
        NodeStyle::BLOCK
            .with_height(20.0)
            .with_margin(Edges::new(30.0, 0.0, 0.0, 0.0)),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    // The anonymous block of text sits between the blocks: its top edge is after the
    // first bottom margin (10 + 20 = 30), it is one 19.2px line tall, and only then
    // does the second block's own top margin apply.
    assert_close(box_of(&tree, between).rect.origin.y, 30.0);
    assert_close(box_of(&tree, second).rect.origin.y, 30.0 + 19.2 + 30.0);
    assert_close(
        box_of(&tree, body).content.height(),
        30.0 + 19.2 + 30.0 + 20.0,
    );
}

#[test]
fn nested_blocks_offset_by_padding_border_and_margins() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let child = element(&mut document, body, "div");
    let mut styles = MockStyles::new();
    styles.set(
        body,
        NodeStyle::BLOCK
            .with_padding(Edges::all(10.0))
            .with_border(Edges::all(5.0)),
    );
    styles.set(
        child,
        NodeStyle::BLOCK
            .with_height(50.0)
            .with_margin(Edges::all(20.0)),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let body_box = box_of(&tree, body);
    // 800 - 2*10 padding - 2*5 border of inner width.
    assert_close(body_box.content.width(), 770.0);
    assert_eq!(body_box.content.origin, jkva_layout::Point::new(15.0, 15.0));

    let child_box = box_of(&tree, child);
    // Content left 15 + left margin 20, content top 15 + top margin 20.
    assert_eq!(child_box.rect.origin, jkva_layout::Point::new(35.0, 35.0));
    // 770 - 2*20 margins.
    assert_close(child_box.rect.width(), 730.0);
    assert_close(child_box.rect.height(), 50.0);

    // The child's margins stay inside the parent's content box (documented
    // simplification: parent/child margin collapsing is not modelled).
    assert_close(body_box.content.height(), 20.0 + 50.0 + 20.0);
    assert_close(body_box.rect.height(), 90.0 + 2.0 * 10.0 + 2.0 * 5.0);
}

#[test]
fn explicit_height_wins_and_content_overflows() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let child = element(&mut document, body, "div");
    let run = text(&mut document, child, "hi");
    let mut styles = MockStyles::new();
    styles.set(child, NodeStyle::BLOCK.with_height(10.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let child_box = box_of(&tree, child);
    assert_close(child_box.content.height(), 10.0);
    assert_close(child_box.rect.height(), 10.0);
    // The line box does not shrink: M1 has no overflow handling, so it just overflows.
    let run_box = box_of(&tree, run);
    assert_close(run_box.rect.height(), 19.2);
    assert_close(run_box.rect.origin.y, 0.0);
}

#[test]
fn text_wraps_greedily_at_spaces() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let paragraph = text(&mut document, body, "aaa bbb ccc");
    let tree = layout_with(&document, &MockStyles::new(), 80.0, 600.0);

    // 8px per character at 16px. "aaa bbb" is 7 characters = 56px and fits in 80px;
    // adding " ccc" needs 88px, so "ccc" starts a second line.
    let runs = text_runs(&tree);
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].text, "aaa bbb");
    assert_close(runs[0].width, 56.0);
    assert_eq!(runs[0].origin, jkva_layout::Point::new(0.0, 0.0));
    assert_eq!(runs[1].text, "ccc");
    assert_close(runs[1].width, 24.0);
    assert_close(runs[1].origin.y, 19.2);

    // Each fragment is one line box high; the block that holds them is two lines high.
    assert_close(box_of(&tree, paragraph).rect.height(), 19.2);
    assert_close(box_of(&tree, body).content.height(), 38.4);
    // A wrapped text node is fragmented: one box per line, all naming the same node.
    assert_eq!(tree.boxes.iter().filter(|b| b.node == paragraph).count(), 2);
}

#[test]
fn a_word_wider_than_the_line_overflows_instead_of_vanishing() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    text(&mut document, body, "abcdefgh");
    let tree = layout_with(&document, &MockStyles::new(), 40.0, 600.0);

    let runs = text_runs(&tree);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "abcdefgh");
    assert_close(runs[0].width, 64.0);
    assert_close(box_of(&tree, body).content.height(), 19.2);
}

#[test]
fn whitespace_collapses_and_line_edges_are_trimmed() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    text(&mut document, body, "  a   b\n c  ");
    let tree = layout_with(&document, &MockStyles::new(), 800.0, 600.0);

    let runs = text_runs(&tree);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "a b c");
    assert_close(runs[0].width, 40.0);
}

#[test]
fn display_none_skips_the_whole_subtree() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let hidden = element(&mut document, body, "div");
    let hidden_text = text(&mut document, hidden, "hidden");
    let shown = text(&mut document, body, "shown");
    let mut styles = MockStyles::new();
    styles.set(hidden, NodeStyle::none());
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    assert!(tree.box_for(hidden).is_none());
    assert!(tree.box_for(hidden_text).is_none());
    let runs = text_runs(&tree);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "shown");
    assert_eq!(
        box_of(&tree, shown).rect.origin,
        jkva_layout::Point::new(0.0, 0.0)
    );
    assert_eq!(tree.len(), 3);
}

#[test]
fn inline_elements_are_flattened_into_the_block_inline_flow() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let span = element(&mut document, body, "span");
    let inline_text = text(&mut document, span, "inside");
    let mut styles = MockStyles::new();
    // The inline element dresses itself up; none of it may affect layout.
    styles.set(
        span,
        NodeStyle::inline()
            .with_margin(Edges::all(50.0))
            .with_padding(Edges::all(50.0))
            .with_border(Edges::all(50.0))
            .with_width(500.0)
            .with_height(500.0),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    // The span gets its own box covering the text it wraps, but its margins,
    // padding, border and width must not move the flow.
    assert_eq!(tree.len(), 4);
    let span_box = tree.box_for(span).expect("inline box");
    assert!(span_box.text.is_none());
    assert_eq!(span_box.rect.origin, jkva_layout::Point::new(0.0, 0.0));
    assert_close(span_box.rect.width(), 48.0);
    let run = box_of(&tree, inline_text);
    assert_eq!(run.rect.origin, jkva_layout::Point::new(0.0, 0.0));
    assert_close(run.rect.width(), 48.0);
    assert_close(box_of(&tree, body).content.height(), 19.2);
}

#[test]
fn a_block_inside_an_inline_element_is_hoisted_into_block_flow() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let span = element(&mut document, body, "span");
    let first = text(&mut document, span, "before");
    let nested = element(&mut document, span, "div");
    let after = text(&mut document, body, "after");
    let mut styles = MockStyles::new();
    styles.set(span, NodeStyle::inline());
    styles.set(nested, NodeStyle::BLOCK.with_height(10.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    // "before" is one line, the hoisted block follows it, "after" follows the block.
    assert_close(box_of(&tree, first).rect.origin.y, 0.0);
    assert_close(box_of(&tree, nested).rect.origin.y, 19.2);
    assert_close(box_of(&tree, after).rect.origin.y, 29.2);
    assert_close(box_of(&tree, body).content.height(), 19.2 + 10.0 + 19.2);
}

#[test]
fn font_size_drives_line_height_and_advance() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let big = text(&mut document, body, "ab");
    let mut styles = MockStyles::new();
    styles.set(big, NodeStyle::BLOCK.with_font_size(20.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let runs = text_runs(&tree);
    assert_eq!(runs.len(), 1);
    assert_close(runs[0].width, 20.0); // 2 characters * 10px
    assert_close(runs[0].font_size, 20.0);
    // line-height: 1.2 * 20px.
    assert_close(box_of(&tree, big).rect.height(), 24.0);
}

#[test]
fn text_colour_is_carried_to_the_run() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let red = text(&mut document, body, "red");
    let mut styles = MockStyles::new();
    styles.set(red, NodeStyle::BLOCK.with_color([255, 0, 0, 255]));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let runs = text_runs(&tree);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].color, [255, 0, 0, 255]);
    assert_eq!(runs[0].text, "red");
}

#[test]
fn children_are_listed_in_paint_order() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let first = element(&mut document, body, "div");
    let between = text(&mut document, body, "mid");
    let second = element(&mut document, body, "div");
    let mut styles = MockStyles::new();
    styles.set(first, NodeStyle::BLOCK.with_height(10.0));
    styles.set(second, NodeStyle::BLOCK.with_height(10.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let body_box = box_of(&tree, body);
    let order: Vec<NodeId> = body_box
        .children
        .iter()
        .map(|index| tree.boxes[*index].node)
        .collect();
    assert_eq!(order, vec![first, between, second]);
    assert_close(box_of(&tree, between).rect.origin.y, 10.0);
    assert_close(box_of(&tree, second).rect.origin.y, 29.2);
}

#[test]
fn a_parsed_document_lays_out_its_implied_structure() {
    let document = parse_document("<p>Hello</p>");
    // The default mock style is "block, 16px, black", which is enough to lay the
    // implied <html> / <head> / <body> structure out.
    let tree = layout_with(&document, &MockStyles::new(), 800.0, 600.0);

    // root, <html>, <head>, <body>, <p>, one text fragment.
    assert_eq!(tree.len(), 6);
    let runs = text_runs(&tree);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "Hello");
    assert_close(runs[0].width, 40.0);
    // <head> is empty, so it is zero-high but still present.
    let html = document
        .first_child(document.root())
        .expect("implied <html>");
    let head = document.first_child(html).expect("implied <head>");
    assert_close(box_of(&tree, head).rect.height(), 0.0);
}

// ---------------------------------------------------------------------------
// Flex formatting context
//
// Items below are plain `<div>`s with mock styles; a row container is 800px wide,
// the default font size is 16px (8px per character) and a line box is 19.2px tall.
// ---------------------------------------------------------------------------

#[test]
fn justify_content_positions_items_along_the_row() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..3)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_width(100.0).with_height(10.0));
    }
    // Three 100px items in an 800px row leave 500px of free space.
    let cases = [
        (JustifyContent::FlexStart, [0.0, 100.0, 200.0]),
        (JustifyContent::FlexEnd, [500.0, 600.0, 700.0]),
        (JustifyContent::Center, [250.0, 350.0, 450.0]),
        (JustifyContent::SpaceBetween, [0.0, 350.0, 700.0]),
        (
            JustifyContent::SpaceAround,
            // leading + 2 * (item + between), with leading = free/(2n) and
            // between = free/n.
            [
                500.0 / 6.0,
                500.0 / 6.0 + 100.0 + 500.0 / 3.0,
                500.0 / 6.0 + 2.0 * (100.0 + 500.0 / 3.0),
            ],
        ),
    ];
    for (justify, expected) in cases {
        styles.set(container, NodeStyle::flex_row().with_justify(justify));
        let tree = layout_with(&document, &styles, 800.0, 600.0);
        assert_positions(xs(&tree, &items), &expected);
        for item in &items {
            let boxed = box_of(&tree, *item);
            assert_close(boxed.rect.width(), 100.0);
            assert_close(boxed.rect.height(), 10.0);
        }
        // One row: the container's auto height is the tallest item.
        assert_close(box_of(&tree, container).content.height(), 10.0);
        assert_eq!(box_of(&tree, container).children.len(), 3);
    }
}

#[test]
fn a_gap_is_counted_once_between_items() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..3)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row().with_gap(10.0));
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_width(100.0).with_height(10.0));
    }
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    // 800 - 3*100 - 2*10 = 480 free, packed at the start.
    assert_positions(xs(&tree, &items), &[0.0, 110.0, 220.0]);
    let middle = box_of(&tree, items[1]).rect.origin.x;
    assert_close(middle - (box_of(&tree, items[0]).rect.max_x()), 10.0);
    assert_close(box_of(&tree, container).content.height(), 10.0);
}

#[test]
fn column_stacks_items_and_applies_the_gap() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let first = element(&mut document, container, "div");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_column().with_gap(5.0));
    styles.set(first, NodeStyle::BLOCK.with_height(10.0));
    styles.set(second, NodeStyle::BLOCK.with_height(20.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    assert_positions(ys(&tree, &[first, second]), &[0.0, 15.0]);
    // Column items stretch to the container's content width by default.
    assert_close(box_of(&tree, first).rect.width(), 800.0);
    assert_close(box_of(&tree, second).rect.width(), 800.0);
    assert_close(box_of(&tree, container).content.height(), 35.0);
}

#[test]
fn column_justify_content_distributes_a_definite_height() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..2)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_height(10.0));
    }
    // 100 - 2*10 = 80 free along the main (vertical) axis.
    let cases = [
        (JustifyContent::FlexStart, [0.0, 10.0]),
        (JustifyContent::FlexEnd, [80.0, 90.0]),
        (JustifyContent::Center, [40.0, 50.0]),
        (JustifyContent::SpaceBetween, [0.0, 90.0]),
        (JustifyContent::SpaceAround, [20.0, 70.0]),
    ];
    for (justify, expected) in cases {
        styles.set(
            container,
            NodeStyle::flex_column()
                .with_height(100.0)
                .with_justify(justify),
        );
        let tree = layout_with(&document, &styles, 800.0, 600.0);
        assert_positions(ys(&tree, &items), &expected);
        assert_close(box_of(&tree, container).content.height(), 100.0);
    }
}

#[test]
fn align_self_overrides_the_container_alignment() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let first = element(&mut document, container, "div");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row().with_height(100.0));
    styles.set(
        first,
        NodeStyle::BLOCK
            .with_width(10.0)
            .with_height(20.0)
            .with_align_self(AlignItems::FlexEnd),
    );
    styles.set(
        second,
        NodeStyle::BLOCK
            .with_width(10.0)
            .with_height(20.0)
            .with_align_self(AlignItems::Center),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);
    assert_positions(ys(&tree, &[first, second]), &[80.0, 40.0]);
}

#[test]
fn align_items_center_aligns_on_the_cross_axis() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let short = element(&mut document, container, "div");
    let tall = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::flex_row()
            .with_align(AlignItems::Center)
            .with_height(100.0),
    );
    styles.set(short, NodeStyle::BLOCK.with_width(100.0).with_height(10.0));
    styles.set(tall, NodeStyle::BLOCK.with_width(100.0).with_height(30.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    // (100 - 10) / 2 = 45 and (100 - 30) / 2 = 35.
    assert_positions(ys(&tree, &[short, tall]), &[45.0, 35.0]);
    assert_close(box_of(&tree, container).content.height(), 100.0);
}

#[test]
fn align_items_stretch_gives_the_line_cross_size() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let with_text = element(&mut document, container, "div");
    let text_node = text(&mut document, with_text, "Hi");
    let empty = element(&mut document, container, "div");
    let fixed = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    styles.set(empty, NodeStyle::BLOCK.with_width(100.0));
    styles.set(fixed, NodeStyle::BLOCK.with_width(10.0).with_height(5.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    // The text item is the tallest at 19.2px, so the line is 19.2px tall.
    assert_positions(xs(&tree, &[with_text, empty, fixed]), &[0.0, 16.0, 116.0]);
    assert_close(box_of(&tree, with_text).rect.height(), 19.2);
    // Stretched from its auto height of 0.
    assert_close(box_of(&tree, empty).rect.height(), 19.2);
    // A declared height is not overridden by stretch.
    assert_close(box_of(&tree, fixed).rect.height(), 5.0);
    assert_close(box_of(&tree, fixed).rect.origin.y, 0.0);
    assert_close(box_of(&tree, container).content.height(), 19.2);
    assert_eq!(
        text_runs(&tree)
            .iter()
            .filter(|run| run.text == "Hi")
            .count(),
        1
    );
    assert!(tree.box_for(text_node).is_some());
}

#[test]
fn an_explicit_width_is_honoured_in_a_row() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let item = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    styles.set(item, NodeStyle::BLOCK.with_width(120.0).with_height(10.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    assert_close(box_of(&tree, item).rect.width(), 120.0);
    assert_close(box_of(&tree, item).rect.origin.x, 0.0);
}

#[test]
fn an_auto_width_comes_from_the_max_content_text_approximation() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let hello = element(&mut document, container, "div");
    text(&mut document, hello, "Hello");
    let hi = element(&mut document, container, "div");
    text(&mut document, hi, "Hi");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    // 5 characters * 8px and 2 characters * 8px, laid out as one line each.
    assert_close(box_of(&tree, hello).rect.width(), 40.0);
    assert_close(box_of(&tree, hi).rect.width(), 16.0);
    assert_positions(xs(&tree, &[hello, hi]), &[0.0, 40.0]);
    assert_close(box_of(&tree, hello).content.height(), 19.2);
    assert_close(box_of(&tree, container).content.height(), 19.2);
}

#[test]
fn an_empty_flex_container_lays_out_no_items() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let boxed = box_of(&tree, container);
    assert!(boxed.children.is_empty());
    assert_close(boxed.content.height(), 0.0);
    assert_close(boxed.rect.height(), 0.0);

    // A declared height still wins, because it does not come from the items.
    styles.set(container, NodeStyle::flex_row().with_height(50.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);
    let boxed = box_of(&tree, container);
    assert!(boxed.children.is_empty());
    assert_close(boxed.content.height(), 50.0);
    assert_close(boxed.rect.height(), 50.0);
}

#[test]
fn a_single_item_is_positioned_by_justify_content() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let item = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::flex_row().with_justify(JustifyContent::Center),
    );
    styles.set(item, NodeStyle::BLOCK.with_width(100.0).with_height(10.0));

    // (800 - 100) / 2 = 350.
    let tree = layout_with(&document, &styles, 800.0, 600.0);
    assert_positions(xs(&tree, &[item]), &[350.0]);

    // `space-between` with one item packs it at the start, as CSS requires.
    styles.set(
        container,
        NodeStyle::flex_row().with_justify(JustifyContent::SpaceBetween),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);
    assert_positions(xs(&tree, &[item]), &[0.0]);
}

#[test]
fn bare_text_inside_a_flex_container_becomes_an_anonymous_item() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let bare = text(&mut document, container, "Hi");
    let block = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    styles.set(block, NodeStyle::BLOCK.with_width(100.0).with_height(10.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let container_box = box_of(&tree, container);
    assert_eq!(container_box.children.len(), 2);
    let item = &tree.boxes[container_box.children[0]];
    assert_eq!(item.node, bare);
    // The item box is a block box around the text, not the text fragment itself.
    assert!(item.text.is_none());
    assert_close(item.rect.width(), 16.0);
    let fragment = &tree.boxes[item.children[0]];
    assert_eq!(
        fragment.text.as_ref().map(|run| run.text.as_str()),
        Some("Hi")
    );
    // The block item follows the anonymous one, with no gap.
    assert_positions(xs(&tree, &[bare, block]), &[0.0, 16.0]);
    assert_close(box_of(&tree, container).content.height(), 19.2);
}

#[test]
fn an_inline_child_of_a_flex_container_is_blockified() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let span = element(&mut document, container, "span");
    text(&mut document, span, "Hi");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    styles.set(span, NodeStyle::inline());
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let span_box = box_of(&tree, span);
    assert_close(span_box.rect.width(), 16.0);
    assert_close(span_box.rect.height(), 19.2);
    // Blockification yields one item box; no separate inline box is added around it.
    assert_eq!(tree.boxes.iter().filter(|b| b.node == span).count(), 1);
}

#[test]
fn flex_items_respect_margin_padding_and_border() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let item = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    styles.set(
        item,
        NodeStyle::BLOCK
            .with_width(100.0)
            .with_padding(Edges::all(10.0))
            .with_border(Edges::all(5.0))
            .with_margin(Edges::all(7.0)),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let boxed = box_of(&tree, item);
    // The margin box starts at the content origin of the container.
    assert_eq!(boxed.rect.origin, jkva_layout::Point::new(7.0, 7.0));
    assert_close(boxed.rect.width(), 130.0); // 100 + 2*10 + 2*5
    assert_close(boxed.rect.height(), 30.0); // empty content: padding + border
    assert_eq!(boxed.content.origin, jkva_layout::Point::new(22.0, 22.0));
    assert_close(boxed.content.width(), 100.0);
    // The container's auto height is the item's outer height, margins included.
    assert_close(box_of(&tree, container).content.height(), 44.0);
}

#[test]
fn a_flex_row_without_wrap_shrinks_instead_of_wrapping() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..3)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_width(50.0).with_height(10.0));
    }
    // A 100px container cannot hold three 50px items and `flex-shrink` defaults to 1,
    // so the items shrink to a third of the line instead of wrapping.
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    let third = 100.0 / 3.0;
    assert_positions(xs(&tree, &items), &[0.0, third, 2.0 * third]);
    for item in &items {
        assert_close(box_of(&tree, *item).rect.width(), third);
    }
    assert_close(box_of(&tree, container).content.height(), 10.0);
}

#[test]
fn a_flex_container_can_be_a_flex_item() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let outer = element(&mut document, body, "div");
    let inner = element(&mut document, outer, "div");
    let first = element(&mut document, inner, "div");
    let second = element(&mut document, inner, "div");
    let mut styles = MockStyles::new();
    styles.set(outer, NodeStyle::flex_row());
    styles.set(inner, NodeStyle::flex_column().with_width(200.0));
    styles.set(first, NodeStyle::BLOCK.with_height(10.0));
    styles.set(second, NodeStyle::BLOCK.with_height(10.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let inner_box = box_of(&tree, inner);
    assert_close(inner_box.rect.width(), 200.0);
    assert_close(inner_box.content.height(), 20.0);
    assert_positions(ys(&tree, &[first, second]), &[0.0, 10.0]);
    // The outer row's auto height comes from the inner container's auto height.
    assert_close(box_of(&tree, outer).content.height(), 20.0);
}

#[test]
fn flex_children_are_pushed_in_paint_order() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..3)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_width(10.0).with_height(10.0));
    }
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    let children = &box_of(&tree, container).children;
    assert_eq!(children.len(), items.len());
    let order: Vec<NodeId> = children
        .iter()
        .map(|index| tree.boxes[*index].node)
        .collect();
    assert_eq!(order, items);
    // Item boxes are appended in order, so the indices ascend.
    assert!(children.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn whitespace_between_flex_items_does_not_create_an_item() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let first = element(&mut document, container, "div");
    // The whitespace an HTML author leaves between elements is a real text node.
    let whitespace = text(&mut document, container, "\n  ");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row().with_gap(10.0));
    styles.set(first, NodeStyle::BLOCK.with_width(100.0).with_height(10.0));
    styles.set(second, NodeStyle::BLOCK.with_width(100.0).with_height(10.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    // No anonymous item and no gap for the whitespace: 0 then 100 + 10.
    assert_eq!(box_of(&tree, container).children.len(), 2);
    assert!(tree.box_for(whitespace).is_none());
    assert_positions(xs(&tree, &[first, second]), &[0.0, 110.0]);
}

#[test]
fn align_items_flex_end_and_flex_start_on_a_row() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let short = element(&mut document, container, "div");
    let tall = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(short, NodeStyle::BLOCK.with_width(10.0).with_height(10.0));
    styles.set(tall, NodeStyle::BLOCK.with_width(10.0).with_height(30.0));

    styles.set(
        container,
        NodeStyle::flex_row()
            .with_align(AlignItems::FlexEnd)
            .with_height(100.0),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);
    // The 10px item sits on the line's bottom edge: 100 - 10.
    assert_positions(ys(&tree, &[short, tall]), &[90.0, 70.0]);

    styles.set(
        container,
        NodeStyle::flex_row().with_align(AlignItems::FlexStart),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);
    assert_positions(ys(&tree, &[short, tall]), &[0.0, 0.0]);
}

#[test]
fn column_items_are_aligned_horizontally() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let item = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(item, NodeStyle::BLOCK.with_width(100.0).with_height(10.0));

    for (align, expected) in [
        (AlignItems::FlexStart, 0.0),
        (AlignItems::Center, 350.0),
        (AlignItems::FlexEnd, 700.0),
    ] {
        styles.set(container, NodeStyle::flex_column().with_align(align));
        let tree = layout_with(&document, &styles, 800.0, 600.0);
        assert_positions(xs(&tree, &[item]), &[expected]);
        // A declared width wins over the column's stretch fallback.
        assert_close(box_of(&tree, item).rect.width(), 100.0);
    }
}

#[test]
fn align_items_stretch_fills_a_definite_cross_size() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let item = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    // The row container has a declared height, so the line's cross size is definite.
    styles.set(container, NodeStyle::flex_row().with_height(100.0));
    styles.set(item, NodeStyle::BLOCK.with_width(10.0));
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    assert_close(box_of(&tree, item).rect.height(), 100.0);
    assert_close(box_of(&tree, item).content.height(), 100.0);
    assert_close(box_of(&tree, container).content.height(), 100.0);
}

// ---------------------------------------------------------------------------
// Flex lines, grow, shrink and basis
// ---------------------------------------------------------------------------

#[test]
fn flex_wrap_breaks_lines_and_counts_the_gap_once() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..3)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row().with_wrap().with_gap(10.0));
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_width(40.0).with_height(10.0));
    }
    // 100px container: two 40px items plus a 10px gap fit (90), the third wraps.
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    assert_positions(xs(&tree, &items), &[0.0, 50.0, 0.0]);
    assert_positions(ys(&tree, &items), &[0.0, 0.0, 20.0]);
    // The gap separates the items on the first line...
    assert_close(
        box_of(&tree, items[1]).rect.origin.x - box_of(&tree, items[0]).rect.max_x(),
        10.0,
    );
    // ...and the two lines: 10 + 10 + 10.
    assert_close(box_of(&tree, container).content.height(), 30.0);
}

#[test]
fn grow_splits_free_space_equally() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let first = element(&mut document, container, "div");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    for item in [first, second] {
        styles.set(
            item,
            NodeStyle::BLOCK
                .with_width(20.0)
                .with_height(10.0)
                .with_grow(1.0),
        );
    }
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    // 2 * 20px base leaves 60px, split 30/30.
    assert_close(box_of(&tree, first).rect.width(), 50.0);
    assert_close(box_of(&tree, second).rect.width(), 50.0);
    assert_positions(xs(&tree, &[first, second]), &[0.0, 50.0]);
    assert_close(box_of(&tree, container).content.height(), 10.0);
}

#[test]
fn grow_zero_gives_everything_to_the_second_item() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let fixed = element(&mut document, container, "div");
    let growing = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    styles.set(fixed, NodeStyle::BLOCK.with_width(20.0).with_height(10.0));
    styles.set(
        growing,
        NodeStyle::BLOCK
            .with_width(20.0)
            .with_height(10.0)
            .with_grow(1.0),
    );
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    assert_close(box_of(&tree, fixed).rect.width(), 20.0);
    assert_close(box_of(&tree, growing).rect.width(), 80.0);
    assert_positions(xs(&tree, &[fixed, growing]), &[0.0, 20.0]);
}

#[test]
fn shrink_halves_an_overflowing_pair() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let first = element(&mut document, container, "div");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    for item in [first, second] {
        // `flex-shrink` defaults to 1.
        styles.set(item, NodeStyle::BLOCK.with_width(80.0).with_height(10.0));
    }
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    // 160px of base size shrinks by 60px, 30px from each equal-weight item.
    assert_close(box_of(&tree, first).rect.width(), 50.0);
    assert_close(box_of(&tree, second).rect.width(), 50.0);
    assert_positions(xs(&tree, &[first, second]), &[0.0, 50.0]);
}

#[test]
fn shrink_zero_keeps_the_overflow() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let first = element(&mut document, container, "div");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    for item in [first, second] {
        styles.set(
            item,
            NodeStyle::BLOCK
                .with_width(80.0)
                .with_height(10.0)
                .with_shrink(0.0),
        );
    }
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    assert_close(box_of(&tree, first).rect.width(), 80.0);
    assert_close(box_of(&tree, second).rect.width(), 80.0);
    assert_positions(xs(&tree, &[first, second]), &[0.0, 80.0]);
    assert_close(box_of(&tree, container).content.height(), 10.0);
}

#[test]
fn zero_basis_with_grow_shares_the_line_equally() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let long = element(&mut document, container, "div");
    text(&mut document, long, "Hello");
    let short = element(&mut document, container, "div");
    text(&mut document, short, "Hi");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    // The different max-content widths must not matter: the basis is zero.
    styles.set(
        long,
        NodeStyle::BLOCK.with_basis(Length::px(0.0)).with_grow(1.0),
    );
    styles.set(
        short,
        NodeStyle::BLOCK.with_basis(Length::px(0.0)).with_grow(1.0),
    );
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    assert_close(box_of(&tree, long).rect.width(), 50.0);
    assert_close(box_of(&tree, short).rect.width(), 50.0);
    assert_positions(xs(&tree, &[long, short]), &[0.0, 50.0]);
}

#[test]
fn flex_basis_beats_the_declared_width() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let item = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    styles.set(
        item,
        NodeStyle::BLOCK
            .with_width(80.0)
            .with_height(10.0)
            .with_basis(Length::px(30.0)),
    );
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    assert_close(box_of(&tree, item).rect.width(), 30.0);
}

#[test]
fn percent_basis_resolves_against_the_container_width() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let first = element(&mut document, container, "div");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    for item in [first, second] {
        styles.set(
            item,
            NodeStyle::BLOCK
                .with_height(10.0)
                .with_basis(Length::percent(50.0)),
        );
    }
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    assert_close(box_of(&tree, first).rect.width(), 50.0);
    assert_close(box_of(&tree, second).rect.width(), 50.0);
}

#[test]
fn each_line_gets_its_own_justify_content_and_align_items() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let short = element(&mut document, container, "div");
    let tall = element(&mut document, container, "div");
    let alone = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::flex_row()
            .with_wrap()
            .with_align(AlignItems::Center)
            .with_justify(JustifyContent::FlexEnd),
    );
    styles.set(short, NodeStyle::BLOCK.with_width(40.0).with_height(10.0));
    styles.set(tall, NodeStyle::BLOCK.with_width(40.0).with_height(30.0));
    styles.set(alone, NodeStyle::BLOCK.with_width(30.0).with_height(10.0));
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    // Line one is 80px wide and 30px tall; line two holds the third item.
    assert_positions(xs(&tree, &[short, tall, alone]), &[20.0, 60.0, 70.0]);
    assert_positions(ys(&tree, &[short, tall, alone]), &[10.0, 0.0, 30.0]);
    assert_close(box_of(&tree, container).content.height(), 40.0);
}

#[test]
fn grow_applies_per_line() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..3)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row().with_wrap());
    for item in &items {
        styles.set(
            *item,
            NodeStyle::BLOCK
                .with_width(40.0)
                .with_height(10.0)
                .with_grow(1.0),
        );
    }
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    // Line one has two items and 20px of free space, line two one item and 60px.
    assert_positions(xs(&tree, &items), &[0.0, 50.0, 0.0]);
    assert_positions(ys(&tree, &items), &[0.0, 0.0, 10.0]);
    assert_close(box_of(&tree, items[0]).rect.width(), 50.0);
    assert_close(box_of(&tree, items[1]).rect.width(), 50.0);
    assert_close(box_of(&tree, items[2]).rect.width(), 100.0);
    assert_close(box_of(&tree, container).content.height(), 20.0);
}

#[test]
fn shrink_never_goes_below_a_zero_content_width() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let first = element(&mut document, container, "div");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    for item in [first, second] {
        // 10px of flex base plus 120px of horizontal padding: the line must shrink the
        // content below zero, which is clamped.
        styles.set(
            item,
            NodeStyle::BLOCK
                .with_basis(Length::px(10.0))
                .with_padding(Edges::new(0.0, 60.0, 0.0, 60.0)),
        );
    }
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    assert_close(box_of(&tree, first).content.width(), 0.0);
    assert_close(box_of(&tree, second).content.width(), 0.0);
    // The padding still occupies the border box.
    assert_close(box_of(&tree, first).rect.width(), 120.0);
    assert_positions(xs(&tree, &[first, second]), &[0.0, 120.0]);
}

#[test]
fn column_wrap_is_unsupported_and_keeps_a_single_line() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..3)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_column().with_wrap());
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_height(30.0));
    }
    // `flex-wrap` is documented as row-only: the column keeps one line.
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    assert_positions(ys(&tree, &items), &[0.0, 30.0, 60.0]);
    assert_positions(xs(&tree, &items), &[0.0, 0.0, 0.0]);
    assert_close(box_of(&tree, container).content.height(), 90.0);
}

// ---------------------------------------------------------------------------
// `order` and `align-content`
// ---------------------------------------------------------------------------

#[test]
fn order_sorts_items_and_keeps_equal_orders_stable() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let last = element(&mut document, container, "div");
    let first = element(&mut document, container, "div");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row());
    styles.set(
        last,
        NodeStyle::BLOCK
            .with_width(10.0)
            .with_height(10.0)
            .with_order(2),
    );
    styles.set(
        first,
        NodeStyle::BLOCK
            .with_width(10.0)
            .with_height(10.0)
            .with_order(0),
    );
    styles.set(
        second,
        NodeStyle::BLOCK
            .with_width(10.0)
            .with_height(10.0)
            .with_order(0),
    );
    let tree = layout_with(&document, &styles, 800.0, 600.0);

    // `first` and `second` share order 0, so document order breaks the tie.
    assert_positions(xs(&tree, &[first, second, last]), &[0.0, 10.0, 20.0]);
    // The children of the container follow the visual order, not document order.
    let children: Vec<NodeId> = box_of(&tree, container)
        .children
        .iter()
        .map(|index| tree.boxes[*index].node)
        .collect();
    assert_eq!(children, vec![first, second, last]);
}

#[test]
fn order_is_applied_before_wrapping() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let first = element(&mut document, container, "div");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(container, NodeStyle::flex_row().with_wrap());
    // `first` has a larger order, so it wraps after `second`.
    styles.set(
        first,
        NodeStyle::BLOCK
            .with_width(60.0)
            .with_height(10.0)
            .with_order(1),
    );
    styles.set(
        second,
        NodeStyle::BLOCK
            .with_width(60.0)
            .with_height(10.0)
            .with_order(0),
    );
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    assert_positions(ys(&tree, &[second, first]), &[0.0, 10.0]);
    assert_close(box_of(&tree, container).content.height(), 20.0);
}

#[test]
fn align_content_modes_position_two_lines_in_a_definite_container() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..4)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_width(50.0).with_height(10.0));
    }
    // Two 10px lines in a 100px container: 80px of cross space to distribute.
    let cases = [
        (AlignContent::FlexStart, [0.0, 0.0, 10.0, 10.0]),
        (AlignContent::FlexEnd, [80.0, 80.0, 90.0, 90.0]),
        (AlignContent::Center, [40.0, 40.0, 50.0, 50.0]),
        (AlignContent::SpaceBetween, [0.0, 0.0, 90.0, 90.0]),
        (AlignContent::SpaceAround, [20.0, 20.0, 70.0, 70.0]),
        // `stretch` grows both lines to 50px each instead of moving them.
        (AlignContent::Stretch, [0.0, 0.0, 50.0, 50.0]),
    ];
    for (align_content, expected) in cases {
        styles.set(
            container,
            NodeStyle::flex_row()
                .with_wrap()
                .with_height(100.0)
                .with_align_content(align_content),
        );
        let tree = layout_with(&document, &styles, 100.0, 600.0);
        assert_positions(ys(&tree, &items), &expected);
        assert_close(box_of(&tree, container).content.height(), 100.0);
        for item in &items {
            // A declared height is never stretched, even in `stretch` mode.
            assert_close(box_of(&tree, *item).rect.height(), 10.0);
        }
    }
}

#[test]
fn align_content_stretch_fills_the_container_height() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..4)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    // No declared heights: the lines start at zero height and stretch fills them.
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_width(50.0));
    }
    styles.set(
        container,
        NodeStyle::flex_row()
            .with_wrap()
            .with_height(100.0)
            .with_align_content(AlignContent::Stretch),
    );
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    // 100px split over two lines: each line grows to 50px.
    assert_positions(ys(&tree, &items), &[0.0, 0.0, 50.0, 50.0]);
    for item in &items {
        assert_close(box_of(&tree, *item).rect.height(), 50.0);
    }
    assert_close(box_of(&tree, container).content.height(), 100.0);
}

#[test]
fn align_content_is_ignored_without_a_definite_height() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..4)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_width(50.0).with_height(10.0));
    }
    styles.set(
        container,
        // No height: the container is content-sized, so there is no space to distribute.
        NodeStyle::flex_row()
            .with_wrap()
            .with_align_content(AlignContent::SpaceBetween),
    );
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    assert_positions(ys(&tree, &items), &[0.0, 0.0, 10.0, 10.0]);
    assert_close(box_of(&tree, container).content.height(), 20.0);
}

#[test]
fn align_content_is_ignored_for_a_single_line() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let first = element(&mut document, container, "div");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(first, NodeStyle::BLOCK.with_width(10.0).with_height(10.0));
    styles.set(second, NodeStyle::BLOCK.with_width(10.0).with_height(10.0));
    styles.set(
        container,
        NodeStyle::flex_row()
            .with_height(100.0)
            .with_align_content(AlignContent::Center),
    );
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    // A single line keeps the definite cross size and stays at the top.
    assert_positions(ys(&tree, &[first, second]), &[0.0, 0.0]);
    assert_close(box_of(&tree, container).content.height(), 100.0);
}

// ---------------------------------------------------------------------------
// Grid
// ---------------------------------------------------------------------------

#[test]
fn three_fixed_columns_have_exact_x_positions() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..3)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::grid().with_columns(vec![
            GridTrack::Px(60.0),
            GridTrack::Px(60.0),
            GridTrack::Px(60.0),
        ]),
    );
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_height(10.0));
    }
    let tree = layout_with(&document, &styles, 300.0, 600.0);

    assert_positions(xs(&tree, &items), &[0.0, 60.0, 120.0]);
    assert_positions(ys(&tree, &items), &[0.0, 0.0, 0.0]);
    for item in &items {
        assert_close(box_of(&tree, *item).rect.width(), 60.0);
    }
    assert_close(box_of(&tree, container).content.height(), 10.0);
}

#[test]
fn fr_tracks_share_the_remaining_width_by_factor() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..3)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::grid().with_columns(vec![
            GridTrack::Px(60.0),
            GridTrack::Fr(1.0),
            GridTrack::Fr(2.0),
        ]),
    );
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_height(10.0));
    }
    let tree = layout_with(&document, &styles, 300.0, 600.0);

    // 300 - 60 = 240 shared 1:2.
    assert_positions(xs(&tree, &items), &[0.0, 60.0, 140.0]);
    assert_close(box_of(&tree, items[0]).rect.width(), 60.0);
    assert_close(box_of(&tree, items[1]).rect.width(), 80.0);
    assert_close(box_of(&tree, items[2]).rect.width(), 160.0);
}

#[test]
fn an_auto_column_is_sized_by_its_widest_item() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let text_item = element(&mut document, container, "div");
    text(&mut document, text_item, "Hello");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::grid().with_columns(vec![GridTrack::Auto, GridTrack::Px(50.0)]),
    );
    let tree = layout_with(&document, &styles, 300.0, 600.0);

    // "Hello" is 5 * 8px, so the auto track is 40px wide.
    assert_close(box_of(&tree, text_item).rect.width(), 40.0);
    assert_positions(xs(&tree, &[text_item, second]), &[0.0, 40.0]);
    assert_close(box_of(&tree, second).rect.width(), 50.0);
    assert_close(box_of(&tree, container).content.height(), 19.2);
}

#[test]
fn a_percentage_track_resolves_against_the_container_width() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let first = element(&mut document, container, "div");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::grid().with_columns(vec![GridTrack::Percent(25.0), GridTrack::Percent(50.0)]),
    );
    styles.set(first, NodeStyle::BLOCK.with_height(10.0));
    styles.set(second, NodeStyle::BLOCK.with_height(10.0));
    let tree = layout_with(&document, &styles, 400.0, 600.0);

    assert_close(box_of(&tree, first).rect.width(), 100.0);
    assert_close(box_of(&tree, second).rect.width(), 200.0);
    assert_positions(xs(&tree, &[first, second]), &[0.0, 100.0]);
}

#[test]
fn row_heights_come_from_the_tallest_item() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let short = element(&mut document, container, "div");
    let tall = element(&mut document, container, "div");
    let next_row = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::grid().with_columns(vec![GridTrack::Px(50.0), GridTrack::Px(50.0)]),
    );
    styles.set(short, NodeStyle::BLOCK.with_height(10.0));
    styles.set(tall, NodeStyle::BLOCK.with_height(30.0));
    styles.set(next_row, NodeStyle::BLOCK.with_height(10.0));
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    // Row 0 is 30px tall because of `tall`, row 1 is 10px.
    assert_positions(ys(&tree, &[short, tall, next_row]), &[0.0, 0.0, 30.0]);
    assert_close(box_of(&tree, short).rect.height(), 10.0);
    assert_close(box_of(&tree, container).content.height(), 40.0);
}

#[test]
fn the_gap_separates_rows_and_columns() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let items: Vec<NodeId> = (0..4)
        .map(|_| element(&mut document, container, "div"))
        .collect();
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::grid()
            .with_columns(vec![GridTrack::Px(40.0), GridTrack::Px(40.0)])
            .with_gap(10.0),
    );
    for item in &items {
        styles.set(*item, NodeStyle::BLOCK.with_height(10.0));
    }
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    assert_positions(xs(&tree, &items), &[0.0, 50.0, 0.0, 50.0]);
    assert_positions(ys(&tree, &items), &[0.0, 0.0, 20.0, 20.0]);
    // Two 10px rows plus the 10px row gap.
    assert_close(box_of(&tree, container).content.height(), 30.0);
}

#[test]
fn grid_items_stretch_to_the_cell_unless_they_declare_a_width() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let stretched = element(&mut document, container, "div");
    let narrow = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::grid().with_columns(vec![GridTrack::Px(100.0), GridTrack::Px(100.0)]),
    );
    styles.set(stretched, NodeStyle::BLOCK.with_height(10.0));
    styles.set(narrow, NodeStyle::BLOCK.with_width(40.0).with_height(10.0));
    let tree = layout_with(&document, &styles, 300.0, 600.0);

    assert_close(box_of(&tree, stretched).rect.width(), 100.0);
    assert_close(box_of(&tree, narrow).rect.width(), 40.0);
    // The narrower item stays at the start of its cell.
    assert_positions(xs(&tree, &[stretched, narrow]), &[0.0, 100.0]);
}

#[test]
fn an_empty_grid_has_no_children() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::grid().with_columns(vec![GridTrack::Px(50.0)]),
    );
    let tree = layout_with(&document, &styles, 300.0, 600.0);

    let boxed = box_of(&tree, container);
    assert!(boxed.children.is_empty());
    assert_close(boxed.content.height(), 0.0);
}

#[test]
fn a_single_item_occupies_the_first_cell() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let item = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::grid().with_columns(vec![GridTrack::Px(50.0)]),
    );
    styles.set(item, NodeStyle::BLOCK.with_height(10.0));
    let tree = layout_with(&document, &styles, 300.0, 600.0);

    assert_positions(xs(&tree, &[item]), &[0.0]);
    assert_close(box_of(&tree, item).rect.width(), 50.0);
    assert_close(box_of(&tree, container).content.height(), 10.0);
}

#[test]
fn a_grid_can_be_a_grid_item() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let outer = element(&mut document, body, "div");
    let inner = element(&mut document, outer, "div");
    let first = element(&mut document, inner, "div");
    let second = element(&mut document, inner, "div");
    let mut styles = MockStyles::new();
    styles.set(
        outer,
        NodeStyle::grid().with_columns(vec![GridTrack::Px(200.0)]),
    );
    styles.set(
        inner,
        NodeStyle::grid().with_columns(vec![GridTrack::Px(50.0), GridTrack::Px(50.0)]),
    );
    styles.set(first, NodeStyle::BLOCK.with_height(10.0));
    styles.set(second, NodeStyle::BLOCK.with_height(10.0));
    let tree = layout_with(&document, &styles, 300.0, 600.0);

    let inner_box = box_of(&tree, inner);
    assert_close(inner_box.rect.width(), 200.0);
    assert_close(inner_box.content.height(), 10.0);
    assert_positions(xs(&tree, &[first, second]), &[0.0, 50.0]);
    assert_close(box_of(&tree, outer).content.height(), 10.0);
}

#[test]
fn more_columns_than_items_leaves_empty_cells() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let item = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::grid().with_columns(vec![
            GridTrack::Px(30.0),
            GridTrack::Px(30.0),
            GridTrack::Px(30.0),
        ]),
    );
    styles.set(item, NodeStyle::BLOCK.with_height(10.0));
    let tree = layout_with(&document, &styles, 300.0, 600.0);

    assert_eq!(box_of(&tree, container).children.len(), 1);
    assert_positions(xs(&tree, &[item]), &[0.0]);
    assert_close(box_of(&tree, item).rect.width(), 30.0);
    assert_close(box_of(&tree, container).content.height(), 10.0);
}

#[test]
fn grid_auto_rows_px_sets_the_row_height() {
    let mut document = Document::new();
    let body = body_element(&mut document);
    let container = element(&mut document, body, "div");
    let first = element(&mut document, container, "div");
    let second = element(&mut document, container, "div");
    let mut styles = MockStyles::new();
    styles.set(
        container,
        NodeStyle::grid()
            .with_columns(vec![GridTrack::Px(50.0)])
            .with_auto_rows(GridAutoRows::Px(40.0)),
    );
    let tree = layout_with(&document, &styles, 100.0, 600.0);

    assert_positions(ys(&tree, &[first, second]), &[0.0, 40.0]);
    // Items without a declared height fill the row.
    assert_close(box_of(&tree, first).rect.height(), 40.0);
    assert_close(box_of(&tree, second).rect.height(), 40.0);
    assert_close(box_of(&tree, container).content.height(), 80.0);
}
