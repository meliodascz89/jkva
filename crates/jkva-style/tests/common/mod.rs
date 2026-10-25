//! Shared helpers for the `jkva-style` integration tests.
//!
//! Compiled into each test binary that declares `mod common;`, so helpers used
//! by only one of them are still live code elsewhere.

#![allow(dead_code)]

use jkva_dom::{Document, ElementData, NodeId, parse_document};
use jkva_style::{
    ComputedStyle, ComputedStyleMap, StyleEngine, Stylesheet, matches_any, parse_selector_list,
};

/// Parses an HTML document.
pub fn parse_html(html: &str) -> Document {
    parse_document(html)
}

/// Every element of the document, in tree order.
pub fn elements(document: &Document) -> Vec<NodeId> {
    document
        .descendants(document.root())
        .filter(|node| document.element(*node).is_some())
        .collect()
}

/// The first element whose `id` attribute equals `id`.
pub fn by_id(document: &Document, id: &str) -> NodeId {
    elements(document)
        .into_iter()
        .find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.attribute("id") == Some(id))
        })
        .unwrap_or_else(|| panic!("no element with id {id:?}"))
}

/// The `id` attributes of every element, in tree order.
pub fn ids(document: &Document) -> Vec<String> {
    elements(document)
        .into_iter()
        .filter_map(|node| {
            document
                .element(node)
                .and_then(|element| element.attribute("id"))
                .map(str::to_owned)
        })
        .collect()
}

/// The first element with the given local name.
pub fn by_tag(document: &Document, name: &str) -> NodeId {
    elements(document)
        .into_iter()
        .find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.name() == name)
        })
        .unwrap_or_else(|| panic!("no <{name}> element"))
}

/// Evaluates one selector (as text) against one node.
///
/// Panics when the selector text does not parse, so that a typo in a test cannot
/// silently become "does not match".
pub fn matches_selector(document: &Document, node: NodeId, selector: &str) -> bool {
    let selectors =
        parse_selector_list(selector).unwrap_or_else(|error| panic!("{selector:?}: {error}"));
    matches_any(document, node, &selectors)
}

/// Asserts that a selector matches exactly the elements that carry the given ids.
///
/// Elements without an `id` attribute — including the implied `<html>`, `<head>`
/// and `<body>` — are ignored, so a test can name only what it cares about.
pub fn assert_selects(document: &Document, selector: &str, expected_ids: &[&str]) {
    let mut matched: Vec<String> = elements(document)
        .into_iter()
        .filter(|node| matches_selector(document, *node, selector))
        .filter_map(|node| {
            document
                .element(node)
                .and_then(|element| element.attribute("id"))
                .map(str::to_owned)
        })
        .collect();
    matched.sort();
    let mut expected: Vec<String> = expected_ids.iter().map(|id| (*id).to_owned()).collect();
    expected.sort();
    assert_eq!(matched, expected, "selector {selector:?}");
}

/// Builds an engine from one stylesheet's source text.
pub fn engine_of(css: &str) -> StyleEngine {
    let sheet = Stylesheet::parse(css).unwrap_or_else(|error| panic!("{error}\n{css}"));
    StyleEngine::from_stylesheets([sheet])
}

/// Builds an engine from several stylesheets, in cascade order.
pub fn engine_from(sheets: &[&str]) -> StyleEngine {
    StyleEngine::from_stylesheets(
        sheets
            .iter()
            .map(|css| Stylesheet::parse(css).unwrap_or_else(|error| panic!("{error}\n{css}"))),
    )
}

/// Computes the style of the element with `id`, panicking when it is unstyled.
pub fn style_of(document: &Document, styles: &ComputedStyleMap, id: &str) -> ComputedStyle {
    let node = by_id(document, id);
    styles
        .get(node)
        .unwrap_or_else(|| panic!("no computed style for #{id}"))
        .clone()
}

/// Returns the element data of one node.
pub fn element(document: &Document, node: NodeId) -> &ElementData {
    document.element(node).expect("node should be an element")
}
