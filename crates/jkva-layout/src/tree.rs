//! The output of layout: the viewport and the flat box tree.
//!
//! [`LayoutTree::boxes`] is a vector, not a linked tree, so that paint can walk it in
//! index order, can sort it, and can keep indices stable while it works. A box names
//! its children by index into the same vector; the root box is always index `0`.

use jkva_dom::NodeId;

use crate::geometry::Rect;
use crate::text::TextRun;

/// The viewport a document is laid out for.
///
/// CSS pixels. M1 has no scrollbars, no device pixel ratio and no visual viewport: the
/// layout viewport is the one the caller passes in. A non-positive or non-finite
/// extent is clamped to `0.0` while laying out, so a bogus viewport yields an empty
/// but well-formed box tree rather than a panic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Width of the layout viewport in CSS pixels.
    pub width: f32,
    /// Height of the layout viewport in CSS pixels.
    pub height: f32,
}

impl Viewport {
    /// Creates a viewport from its extents.
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// One box produced by layout.
///
/// A box is a **fragment**: one DOM node can produce several boxes. A block element
/// produces one box; a text node produces one box per line it occupies; a node that is
/// `display: none` produces none. Boxes for text fragments repeat the [`NodeId`] of the
/// text node and carry a [`TextRun`].
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutBox {
    /// The DOM node this box came from.
    ///
    /// For a text fragment this is the text node; for a block box it is the element.
    /// Layout never invents nodes, so every box can be traced back to the document.
    pub node: NodeId,
    /// Border box in absolute document coordinates.
    ///
    /// This is the box the paint layer paints the background and border into. For a
    /// text fragment there is no border or padding, so it equals
    /// [`content`](Self::content).
    pub rect: Rect,
    /// Content box in absolute document coordinates.
    ///
    /// The area inside the border and padding. For a block box it is where its
    /// children live; for a text fragment it equals [`rect`](Self::rect).
    pub content: Rect,
    /// Indices of the child boxes in [`LayoutTree::boxes`], in paint order.
    ///
    /// Children are in document order, with text fragments interleaved where they
    /// appear. A box that only holds text still lists its text fragments here, so a
    /// consumer that walks children sees the text.
    pub children: Vec<usize>,
    /// The line fragment when this box is a text fragment, `None` for a block box.
    pub text: Option<TextRun>,
}

/// The result of laying out one document.
///
/// The tree is flat and owns every box; it borrows nothing from the document or from
/// the style source, so it can outlive both (within the arena limits of ADR-008).
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutTree {
    /// Every box, in creation order. Index `0` is always the document root box.
    pub boxes: Vec<LayoutBox>,
    /// The viewport the tree was laid out for, exactly as passed to
    /// [`layout`](crate::layout).
    pub viewport: Viewport,
}

impl LayoutTree {
    /// Returns the root box, or `None` for a hand-built empty tree.
    ///
    /// [`layout`](crate::layout) always produces a root, so `None` only appears for a
    /// tree a caller assembled itself.
    #[must_use]
    pub fn root(&self) -> Option<&LayoutBox> {
        self.boxes.first()
    }

    /// Returns the number of boxes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.boxes.len()
    }

    /// Returns `true` when the tree holds no boxes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }

    /// Returns the first box generated for `node`, if any.
    ///
    /// This is a linear scan and returns the *first* fragment, which for a wrapped
    /// text node is its first line. A node that is `display: none` has no box.
    #[must_use]
    pub fn box_for(&self, node: NodeId) -> Option<&LayoutBox> {
        self.boxes.iter().find(|b| b.node == node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    #[test]
    fn viewport_keeps_the_values_it_is_given() {
        let viewport = Viewport::new(800.0, 600.0);
        assert_eq!(viewport.width, 800.0);
        assert_eq!(viewport.height, 600.0);
        assert_eq!(
            viewport,
            Viewport {
                width: 800.0,
                height: 600.0
            }
        );
    }

    #[test]
    fn empty_tree_has_no_root_and_reports_its_length() {
        let tree = LayoutTree {
            boxes: Vec::new(),
            viewport: Viewport::new(10.0, 10.0),
        };
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert!(tree.root().is_none());
    }

    #[test]
    fn box_for_finds_the_first_fragment_of_a_node() {
        let mut document = jkva_dom::Document::new();
        let node = document.create_element("div");
        let other = document.create_element("p");
        let tree = LayoutTree {
            boxes: vec![
                LayoutBox {
                    node,
                    rect: Rect::new(0.0, 0.0, 10.0, 10.0),
                    content: Rect::new(0.0, 0.0, 10.0, 10.0),
                    children: Vec::new(),
                    text: None,
                },
                LayoutBox {
                    node: other,
                    rect: Rect::new(0.0, 10.0, 10.0, 10.0),
                    content: Rect::new(0.0, 10.0, 10.0, 10.0),
                    children: Vec::new(),
                    text: None,
                },
            ],
            viewport: Viewport::new(10.0, 20.0),
        };
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.root().map(|b| b.node), Some(node));
        assert_eq!(tree.box_for(node).map(|b| b.rect.origin), Some(Point::ZERO));
        assert_eq!(tree.box_for(other).map(|b| b.rect.origin.y), Some(10.0));
        let absent = document.create_element("span");
        assert!(tree.box_for(absent).is_none());
    }
}
