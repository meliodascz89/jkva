//! `jkva-engine` — the document pipeline that ties the layers together.
//!
//! M1 scope: load a page (HTML + inline `<style>` sheets) and expose the pieces
//! the style, layout and paint stages need. Rendering arrives as the layers land;
//! this crate is the single place where they meet, so no layer has to know about
//! another one's internals.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use jkva_dom::{Document, NodeKind, parse_document};
use jkva_style::Stylesheet;

/// Version of the JKva engine crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A loaded document plus the stylesheets found inside it.
#[derive(Debug)]
pub struct Page {
    /// The parsed DOM.
    pub document: Document,
    /// Author stylesheets in document order (currently only `<style>` blocks).
    pub stylesheets: Vec<Stylesheet>,
    /// Non-fatal problems found while loading, for diagnostics and tests.
    pub warnings: Vec<String>,
}

/// Viewport size in CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Width in CSS pixels.
    pub width: f32,
    /// Height in CSS pixels.
    pub height: f32,
}

impl Viewport {
    /// Creates a viewport.
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new(800.0, 600.0)
    }
}

/// Parses HTML and collects every `<style>` element as an author stylesheet.
///
/// `<link rel=stylesheet>` is not loaded yet: there is no network layer in M1.
/// A malformed stylesheet is reported in [`Page::warnings`] and skipped.
#[must_use]
pub fn parse_page(html: &str) -> Page {
    let document = parse_document(html);
    let mut stylesheets = Vec::new();
    let mut warnings = Vec::new();

    for node in document.descendants(document.root()) {
        let NodeKind::Element(element) = document.kind(node) else {
            continue;
        };
        if element.name() != "style" || !element.is_html() {
            continue;
        }
        let css = document.text_content(node);
        match Stylesheet::parse(&css) {
            Ok(sheet) => stylesheets.push(sheet),
            Err(error) => warnings.push(format!(
                "stylesheet rejected at {:?}: {error}",
                error.position()
            )),
        }
    }

    Page {
        document,
        stylesheets,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_style_blocks_in_order() {
        let page =
            parse_page("<style>p { color: red }</style><style>div { margin: 1px }</style><p>x</p>");
        assert_eq!(page.stylesheets.len(), 2);
        assert_eq!(page.stylesheets[0].rules().len(), 1);
        assert_eq!(page.stylesheets[1].rules().len(), 1);
        assert!(page.warnings.is_empty());
    }

    #[test]
    fn page_without_styles_has_none() {
        let page = parse_page("<p>hello</p>");
        assert!(page.stylesheets.is_empty());
        assert!(page.warnings.is_empty());
    }

    #[test]
    fn viewport_defaults_to_800x600() {
        let viewport = Viewport::default();
        assert_eq!((viewport.width, viewport.height), (800.0, 600.0));
    }
}
