//! End-to-end tests of `jkva_layout::layout` against a mock style source and a
//! deterministic text measurer.
//!
//! Every expected number in this file is derivable by hand from the documented M1
//! model: the mocks charge half an em per character (`font_size * 0.5`), the default
//! font size is 16px, and a line box is `1.2 * font_size` tall. Tests that pin down a
//! simplification say so in a comment, so that a future change to the model has to
//! update them on purpose.

use std::collections::HashMap;

use jkva_dom::{Document, NodeId, parse_document};
use jkva_layout::{
    Display, Edges, LayoutBox, LayoutTree, Length, LengthEdges, Rect, StyleSource, TextMeasure,
    TextRun, Viewport, layout,
};

const BLACK: [u8; 4] = [0, 0, 0, 255];

/// The style of one node as the mock reports it.
#[derive(Clone, Copy, Debug)]
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

    fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    fn with_color(mut self, color: [u8; 4]) -> Self {
        self.color = color;
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
        self.nodes.get(&node).copied().unwrap_or(NodeStyle::BLOCK)
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
