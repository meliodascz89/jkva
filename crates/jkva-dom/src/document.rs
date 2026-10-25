//! The document tree: an arena of nodes plus every operation that links them.
//!
//! [`Document`] is the only type that mutates tree links. Each mutating method
//! leaves the five links of every touched node in a consistent state, so public
//! reads (`parent`, `first_child`, `last_child`, `prev_sibling`,
//! `next_sibling`, [`Document::children`]) never observe a half-attached node.
//!
//! Traversal is deliberately iterative and pointer-free: the tree is a set of
//! indices into one [`Arena`], it is freed in O(1) by dropping the arena, and
//! there are no reference cycles (ADR-008).

use std::fmt;

use html5ever::QualName;
use jkva_core::Arena;

use crate::node::{
    Attributes, Doctype, ElementData, HTML_NAMESPACE, NodeData, NodeId, NodeKind, QuirksMode,
};

/// Upper bound on the number of parser error messages kept per document.
///
/// Parsing hostile markup can produce an unbounded number of diagnostics, so the
/// log keeps the first [`MAX_RETAINED_PARSE_ERRORS`] messages and only counts the
/// rest.
pub const MAX_RETAINED_PARSE_ERRORS: usize = 64;

/// Diagnostics collected while parsing one document.
#[derive(Clone, Debug, Default)]
pub struct ParseErrors {
    total: usize,
    retained: Vec<String>,
}

impl ParseErrors {
    /// Returns how many parse errors the parser reported in total.
    #[must_use]
    pub const fn total_count(&self) -> usize {
        self.total
    }

    /// Returns the retained messages, in the order they were reported.
    ///
    /// At most [`MAX_RETAINED_PARSE_ERRORS`] messages are kept.
    #[must_use]
    pub fn retained(&self) -> &[String] {
        &self.retained
    }

    /// Returns `true` when the parser reported no error at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Records one message, retaining it only while the cap allows.
    pub(crate) fn record(&mut self, message: String) {
        self.total += 1;
        if self.retained.len() < MAX_RETAINED_PARSE_ERRORS {
            self.retained.push(message);
        }
    }
}

/// A parsed HTML document, or a detached document fragment.
///
/// The tree lives in one arena; dropping the document frees everything at once.
/// Handles are generation-checked, so a [`NodeId`] taken before a removal can
/// never resolve to the node that later reused the slot.
pub struct Document {
    nodes: Arena<NodeData>,
    root: NodeId,
    quirks_mode: QuirksMode,
    doctype: Option<Doctype>,
    parse_errors: ParseErrors,
}

impl Document {
    /// Creates an empty document with a single [`NodeKind::Document`] root.
    ///
    /// [`Self::root`] returns the root handle. The root has no parent and never
    /// moves; every other node is reachable from it (except `<template>`
    /// contents fragments, which are owned by their element).
    #[must_use]
    pub fn new() -> Self {
        let mut nodes = Arena::new();
        let root = nodes.insert(NodeData::new(NodeKind::Document));
        Self {
            nodes,
            root,
            quirks_mode: QuirksMode::default(),
            doctype: None,
            parse_errors: ParseErrors::default(),
        }
    }

    /// Returns the handle of the document root.
    #[must_use]
    pub const fn root(&self) -> NodeId {
        self.root
    }

    /// Returns the data of one node.
    ///
    /// # Panics
    ///
    /// Panics when `id` is stale or was never issued by this document. Handles
    /// obtained from this document stay valid until the node is removed.
    #[must_use]
    pub fn node(&self, id: NodeId) -> &NodeData {
        self.expect_node(id)
    }

    /// Returns the data of one node, or `None` when the handle is stale.
    #[must_use]
    pub fn try_node(&self, id: NodeId) -> Option<&NodeData> {
        self.nodes.get(id)
    }

    /// Returns the kind of one node.
    ///
    /// # Panics
    ///
    /// Panics when `id` is stale.
    #[must_use]
    pub fn kind(&self, id: NodeId) -> &NodeKind {
        self.expect_node(id).kind()
    }

    /// Returns the element data of an element node, and `None` for any other
    /// kind of node or for a stale handle.
    #[must_use]
    pub fn element(&self, id: NodeId) -> Option<&ElementData> {
        self.nodes.get(id).and_then(NodeData::as_element)
    }

    /// Returns the parent of one node, or `None` for the root and for detached
    /// nodes.
    ///
    /// # Panics
    ///
    /// Panics when `id` is stale.
    #[must_use]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.expect_node(id).parent()
    }

    /// Returns the first child of one node.
    ///
    /// # Panics
    ///
    /// Panics when `id` is stale.
    #[must_use]
    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.expect_node(id).first_child()
    }

    /// Returns the last child of one node.
    ///
    /// # Panics
    ///
    /// Panics when `id` is stale.
    #[must_use]
    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.expect_node(id).last_child()
    }

    /// Returns the next sibling of one node.
    ///
    /// # Panics
    ///
    /// Panics when `id` is stale.
    #[must_use]
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.expect_node(id).next_sibling()
    }

    /// Returns the previous sibling of one node.
    ///
    /// # Panics
    ///
    /// Panics when `id` is stale.
    #[must_use]
    pub fn prev_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.expect_node(id).prev_sibling()
    }

    /// Iterates over the children of one node in tree order.
    ///
    /// # Panics
    ///
    /// Panics when `id` is stale.
    #[must_use]
    pub fn children(&self, id: NodeId) -> Children<'_> {
        Children {
            document: self,
            next: self.expect_node(id).first_child(),
        }
    }

    /// Iterates over `id` and its descendants in pre-order (document order).
    ///
    /// # Panics
    ///
    /// Panics when `id` is stale.
    #[must_use]
    pub fn descendants(&self, id: NodeId) -> Descendants<'_> {
        self.expect_node(id);
        Descendants {
            document: self,
            stop: id,
            next: Some(id),
        }
    }

    /// Returns the value of one attribute on an element.
    ///
    /// Returns `None` when the handle is stale, the node is not an element, or
    /// the attribute is absent.
    #[must_use]
    pub fn get_attribute(&self, element: NodeId, name: &str) -> Option<&str> {
        self.element(element).and_then(|data| data.attribute(name))
    }

    /// Returns the text content of a node.
    ///
    /// For a text or comment node this is its own data. For the document root and
    /// for elements it is the concatenation of all descendant text nodes in tree
    /// order; comments inside the subtree are excluded, and `<template>` contents
    /// fragments are not visited because they are not children.
    ///
    /// # Panics
    ///
    /// Panics when `id` is stale.
    #[must_use]
    pub fn text_content(&self, id: NodeId) -> String {
        match self.kind(id) {
            NodeKind::Text(text) | NodeKind::Comment(text) => text.clone(),
            NodeKind::Document | NodeKind::Element(_) => {
                let mut out = String::new();
                for node in self.descendants(id) {
                    if let NodeKind::Text(text) = self.expect_node(node).kind() {
                        out.push_str(text);
                    }
                }
                out
            }
        }
    }

    /// Returns the number of live nodes, including detached fragments.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the quirks mode the parser computed from the doctype.
    #[must_use]
    pub const fn quirks_mode(&self) -> QuirksMode {
        self.quirks_mode
    }

    /// Returns the doctype of the document, if it has one.
    #[must_use]
    pub const fn doctype(&self) -> Option<&Doctype> {
        self.doctype.as_ref()
    }

    /// Returns the diagnostics of the last parse.
    #[must_use]
    pub const fn parse_errors(&self) -> &ParseErrors {
        &self.parse_errors
    }

    /// Creates a detached element in the HTML namespace with no attributes.
    ///
    /// The name is lowercased, matching the DOM `createElement` of an HTML
    /// document.
    #[must_use]
    pub fn create_element(&mut self, name: &str) -> NodeId {
        self.create_element_with_attributes(name, Attributes::new())
    }

    /// Creates a detached HTML element with the given attributes.
    ///
    /// The name is lowercased; attributes keep the order of the map.
    #[must_use]
    pub fn create_element_with_attributes(&mut self, name: &str, attributes: Attributes) -> NodeId {
        let local_name = name.to_ascii_lowercase();
        let qual_name = QualName::new(None, HTML_NAMESPACE.into(), local_name.as_str().into());
        self.create_element_qualified(qual_name, attributes, false)
    }

    /// Creates a detached text node.
    #[must_use]
    pub fn create_text(&mut self, text: &str) -> NodeId {
        self.nodes
            .insert(NodeData::new(NodeKind::Text(text.to_owned())))
    }

    /// Creates a detached comment node.
    #[must_use]
    pub fn create_comment(&mut self, text: &str) -> NodeId {
        self.nodes
            .insert(NodeData::new(NodeKind::Comment(text.to_owned())))
    }

    /// Appends `child` as the last child of `parent`.
    ///
    /// If `child` already has a parent it is moved, as DOM `appendChild` does.
    /// Adjacent text nodes are kept apart: only the parser merges them.
    /// Hierarchical constraints (for example "the document root may hold only one
    /// element child") are the API layer's business, not the tree's.
    ///
    /// # Panics
    ///
    /// Panics when either handle is stale, or when `child` is `parent` itself or
    /// one of its ancestors (that would create a cycle).
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.check_can_insert(parent, child);
        if self.expect_node(child).parent().is_some() {
            self.detach(child);
        }
        self.append_detached(parent, child);
    }

    /// Removes `node` and its whole subtree from the document and returns how
    /// many nodes were freed.
    ///
    /// This is DOM `removeChild` plus immediate reclamation: the arena slots are
    /// returned to the free list, so every handle into the removed subtree becomes
    /// stale at once and can never resolve to a node that later reuses the slot.
    /// `<template>` contents fragments are removed with their element, which is the
    /// only way to reclaim them because they are not children.
    ///
    /// # Panics
    ///
    /// Panics when `node` is a stale handle or is the document root.
    pub fn remove_node(&mut self, node: NodeId) -> usize {
        assert_ne!(node, self.root, "the document root cannot be removed");
        self.expect_node(node);
        let mut doomed: Vec<NodeId> = self.descendants(node).collect();
        // Fragments are not reachable from the tree, so grow the list explicitly
        // until every nested template fragment has been collected as well.
        let mut index = 0;
        while index < doomed.len() {
            if let Some(fragment) = self
                .element(doomed[index])
                .and_then(ElementData::template_contents)
            {
                doomed.extend(self.descendants(fragment));
            }
            index += 1;
        }
        self.detach(node);
        doomed.reverse();
        let mut freed = 0;
        for id in doomed {
            if self.nodes.remove(id).is_some() {
                freed += 1;
            }
        }
        freed
    }

    /// Sets one attribute of an element.
    ///
    /// A missing attribute is added at the end; an existing one keeps its
    /// position. The call does nothing when `element` is not an element node.
    ///
    /// # Panics
    ///
    /// Panics when `element` is a stale handle.
    pub fn set_attribute(&mut self, element: NodeId, name: &str, value: &str) {
        self.expect_node(element);
        if let Some(NodeKind::Element(data)) = self.nodes.get_mut(element).map(NodeData::kind_mut) {
            data.attributes_mut().insert(name, value);
        }
    }

    /// Removes one attribute from an element and returns its value.
    ///
    /// Returns `None` when the attribute is absent or `element` is not an element
    /// node.
    ///
    /// # Panics
    ///
    /// Panics when `element` is a stale handle.
    pub fn remove_attribute(&mut self, element: NodeId, name: &str) -> Option<String> {
        self.expect_node(element);
        match self.nodes.get_mut(element).map(NodeData::kind_mut) {
            Some(NodeKind::Element(data)) => data.attributes_mut().remove(name),
            _ => None,
        }
    }

    /// Creates an element from a qualified name, recording the parse-time flag
    /// the HTML tree builder asked for.
    ///
    /// An HTML `<template>` element owns a detached [`NodeKind::Document`]
    /// fragment that receives its children. That rule is the one the HTML parser
    /// uses (HTML namespace plus the local name `template`), so an element built
    /// by the parser and one built by [`Self::create_element`] behave the same.
    pub(crate) fn create_element_qualified(
        &mut self,
        qual_name: QualName,
        attributes: Attributes,
        mathml_annotation_xml_integration_point: bool,
    ) -> NodeId {
        let is_template = &*qual_name.ns == HTML_NAMESPACE && &*qual_name.local == "template";
        let template_contents = if is_template {
            Some(self.nodes.insert(NodeData::new(NodeKind::Document)))
        } else {
            None
        };
        let data = ElementData::new(
            qual_name,
            attributes,
            template_contents,
            mathml_annotation_xml_integration_point,
        );
        self.nodes.insert(NodeData::new(NodeKind::Element(data)))
    }

    /// Appends a node that may still be attached, moving it when needed.
    pub(crate) fn append_moving(&mut self, parent: NodeId, child: NodeId) {
        if self.expect_node(child).parent().is_some() {
            self.detach(child);
        }
        self.append_detached(parent, child);
    }

    /// Inserts a node that may still be attached immediately before `sibling`.
    ///
    /// Does nothing when `sibling` is detached: without a parent there is no
    /// insertion point. The HTML tree builder never asks for that.
    pub(crate) fn insert_before_moving(&mut self, sibling: NodeId, new_child: NodeId) {
        self.expect_node(sibling);
        if self.expect_node(new_child).parent().is_some() {
            self.detach(new_child);
        }
        let Some(parent) = self.expect_node(sibling).parent() else {
            debug_assert!(false, "insert_before_moving on a detached sibling");
            return;
        };
        let previous = self.expect_node(sibling).prev_sibling();
        self.link_siblings(new_child, parent, previous, Some(sibling));
    }

    /// Appends text, merging it into the last child when that is a text node.
    ///
    /// Merging adjacent text is required by the HTML tree builder.
    pub(crate) fn append_text(&mut self, parent: NodeId, text: &str) {
        self.expect_node(parent);
        if let Some(last) = self.expect_node(parent).last_child()
            && let Some(existing) = self.text_mut(last)
        {
            existing.push_str(text);
            return;
        }
        let node = self.create_text(text);
        self.append_detached(parent, node);
    }

    /// Inserts text before `sibling`, merging it into the previous text node when
    /// that is a text node.
    pub(crate) fn insert_text_before(&mut self, sibling: NodeId, text: &str) {
        self.expect_node(sibling);
        if let Some(previous) = self.expect_node(sibling).prev_sibling()
            && let Some(existing) = self.text_mut(previous)
        {
            existing.push_str(text);
            return;
        }
        let node = self.create_text(text);
        self.insert_before_moving(sibling, node);
    }

    /// Detaches a node from its parent and siblings.
    ///
    /// Detaching a detached node, or the root, does nothing.
    pub(crate) fn detach(&mut self, node: NodeId) {
        let Some(data) = self.nodes.get(node) else {
            return;
        };
        let parent = data.parent();
        let previous = data.prev_sibling();
        let next = data.next_sibling();
        if let Some(data) = self.nodes.get_mut(node) {
            data.set_parent(None);
            data.set_prev_sibling(None);
            data.set_next_sibling(None);
        }
        if let Some(previous) = previous
            && let Some(data) = self.nodes.get_mut(previous)
        {
            data.set_next_sibling(next);
        }
        if let Some(next) = next
            && let Some(data) = self.nodes.get_mut(next)
        {
            data.set_prev_sibling(previous);
        }
        if let Some(parent) = parent
            && let Some(data) = self.nodes.get_mut(parent)
        {
            if data.first_child() == Some(node) {
                data.set_first_child(next);
            }
            if data.last_child() == Some(node) {
                data.set_last_child(previous);
            }
        }
    }

    /// Moves every child of `node` to the end of `new_parent`, keeping their
    /// order.
    pub(crate) fn reparent_children(&mut self, node: NodeId, new_parent: NodeId) {
        self.expect_node(node);
        self.expect_node(new_parent);
        let mut child = self.expect_node(node).first_child();
        while let Some(current) = child {
            child = self.expect_node(current).next_sibling();
            self.detach(current);
            self.append_detached(new_parent, current);
        }
    }

    /// Adds attributes to an element, skipping names that are already present.
    pub(crate) fn add_attributes_if_missing(&mut self, element: NodeId, attributes: &Attributes) {
        if let Some(NodeKind::Element(data)) = self.nodes.get_mut(element).map(NodeData::kind_mut) {
            let target = data.attributes_mut();
            for (name, value) in attributes.iter() {
                target.insert_if_missing(name, value);
            }
        }
    }

    /// Records the quirks mode the tree builder computed.
    pub(crate) fn set_quirks_mode(&mut self, mode: QuirksMode) {
        self.quirks_mode = mode;
    }

    /// Records the doctype of the document.
    pub(crate) fn set_doctype(&mut self, doctype: Doctype) {
        self.doctype = Some(doctype);
    }

    /// Records one parser diagnostic.
    pub(crate) fn record_parse_error(&mut self, message: String) {
        self.parse_errors.record(message);
    }

    /// Returns the qualified name of an element, panicking when the handle does
    /// not point at an element.
    ///
    /// Used by the tree builder, which only ever asks about elements it created.
    pub(crate) fn element_data(&self, id: NodeId) -> &ElementData {
        self.element(id)
            .unwrap_or_else(|| panic!("node {id:?} is not a live element"))
    }

    /// Returns the data of one node and panics when the handle is stale.
    fn expect_node(&self, id: NodeId) -> &NodeData {
        self.nodes
            .get(id)
            .unwrap_or_else(|| panic!("stale NodeId {id:?}"))
    }

    /// Returns the payload of a text node for mutation.
    fn text_mut(&mut self, id: NodeId) -> Option<&mut String> {
        match self.nodes.get_mut(id)?.kind_mut() {
            NodeKind::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Panics unless `child` may be inserted below `parent`.
    fn check_can_insert(&self, parent: NodeId, child: NodeId) {
        self.expect_node(parent);
        self.expect_node(child);
        assert_ne!(parent, child, "cannot insert a node into itself");
        let mut cursor = Some(parent);
        while let Some(node) = cursor {
            assert_ne!(node, child, "cannot insert a node into its own subtree");
            cursor = self.expect_node(node).parent();
        }
    }

    /// Links an already detached `child` as the last child of `parent`.
    fn append_detached(&mut self, parent: NodeId, child: NodeId) {
        self.expect_node(parent);
        self.expect_node(child);
        let last = self.expect_node(parent).last_child();
        self.link_siblings(child, parent, last, None);
    }

    /// Wires `node` between `previous` and `next` under `parent`.
    ///
    /// The caller guarantees that `node` is detached and that `previous` and
    /// `next` are the neighbours it should end up between.
    fn link_siblings(
        &mut self,
        node: NodeId,
        parent: NodeId,
        previous: Option<NodeId>,
        next: Option<NodeId>,
    ) {
        if let Some(data) = self.nodes.get_mut(node) {
            data.set_parent(Some(parent));
            data.set_prev_sibling(previous);
            data.set_next_sibling(next);
        }
        match previous {
            Some(previous) => {
                if let Some(data) = self.nodes.get_mut(previous) {
                    data.set_next_sibling(Some(node));
                }
            }
            None => {
                if let Some(data) = self.nodes.get_mut(parent) {
                    data.set_first_child(Some(node));
                }
            }
        }
        match next {
            Some(next) => {
                if let Some(data) = self.nodes.get_mut(next) {
                    data.set_prev_sibling(Some(node));
                }
            }
            None => {
                if let Some(data) = self.nodes.get_mut(parent) {
                    data.set_last_child(Some(node));
                }
            }
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Document")
            .field("root", &self.root)
            .field("nodes", &self.nodes.len())
            .field("quirks_mode", &self.quirks_mode)
            .finish_non_exhaustive()
    }
}

/// Iterator over the children of one node, in tree order.
pub struct Children<'a> {
    document: &'a Document,
    next: Option<NodeId>,
}

impl Iterator for Children<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let current = self.next?;
        self.next = self.document.try_node(current)?.next_sibling();
        Some(current)
    }
}

/// Iterator over one node and its descendants, in pre-order (document order).
///
/// The traversal never leaves the subtree rooted at the node it was created from.
pub struct Descendants<'a> {
    document: &'a Document,
    stop: NodeId,
    next: Option<NodeId>,
}

impl Iterator for Descendants<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let current = self.next?;
        let data = self.document.try_node(current)?;
        let mut upcoming = data.first_child();
        if upcoming.is_none() {
            let mut cursor = current;
            while cursor != self.stop {
                match self.document.try_node(cursor) {
                    Some(node) if node.next_sibling().is_some() => {
                        upcoming = node.next_sibling();
                        break;
                    }
                    Some(node) => match node.parent() {
                        Some(parent) => cursor = parent,
                        None => break,
                    },
                    None => break,
                }
            }
        }
        self.next = upcoming;
        Some(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(document: &Document, id: NodeId) -> Option<String> {
        document
            .try_node(id)
            .and_then(NodeData::text)
            .map(str::to_owned)
    }

    #[test]
    fn new_document_has_a_root_and_nothing_else() {
        let document = Document::new();
        assert_eq!(document.node_count(), 1);
        assert!(matches!(document.kind(document.root()), NodeKind::Document));
        assert_eq!(document.parent(document.root()), None);
        assert_eq!(document.children(document.root()).count(), 0);
        assert_eq!(document.quirks_mode(), QuirksMode::NoQuirks);
        assert!(document.doctype().is_none());
        assert!(document.parse_errors().is_empty());
    }

    #[test]
    fn append_child_links_parent_siblings_and_children() {
        let mut document = Document::new();
        let root = document.root();
        let div = document.create_element("div");
        let a = document.create_element("a");
        let b = document.create_element("b");
        document.append_child(root, div);
        document.append_child(div, a);
        document.append_child(div, b);

        assert_eq!(document.first_child(div), Some(a));
        assert_eq!(document.last_child(div), Some(b));
        assert_eq!(document.next_sibling(a), Some(b));
        assert_eq!(document.prev_sibling(b), Some(a));
        assert_eq!(document.prev_sibling(a), None);
        assert_eq!(document.next_sibling(b), None);
        assert_eq!(document.parent(a), Some(div));
        assert_eq!(document.parent(div), Some(root));
        let children: Vec<_> = document.children(div).collect();
        assert_eq!(children, vec![a, b]);
    }

    #[test]
    fn append_child_moves_an_attached_node() {
        let mut document = Document::new();
        let root = document.root();
        let left = document.create_element("left");
        let right = document.create_element("right");
        let moved = document.create_element("moved");
        document.append_child(root, left);
        document.append_child(root, right);
        document.append_child(left, moved);
        document.append_child(right, moved);

        assert_eq!(document.children(left).count(), 0);
        assert_eq!(document.first_child(right), Some(moved));
        assert_eq!(document.parent(moved), Some(right));
        assert_eq!(document.last_child(root), Some(right));
    }

    #[test]
    fn insert_before_moving_keeps_sibling_links_consistent() {
        let mut document = Document::new();
        let root = document.root();
        let first = document.create_element("first");
        let last = document.create_element("last");
        let middle = document.create_element("middle");
        document.append_child(root, first);
        document.append_child(root, last);
        document.insert_before_moving(last, middle);

        let children: Vec<_> = document.children(root).collect();
        assert_eq!(children, vec![first, middle, last]);
        assert_eq!(document.next_sibling(first), Some(middle));
        assert_eq!(document.prev_sibling(last), Some(middle));
    }

    #[test]
    fn detach_repairs_neighbours_and_the_parent() {
        let mut document = Document::new();
        let root = document.root();
        let a = document.create_element("a");
        let b = document.create_element("b");
        let c = document.create_element("c");
        for node in [a, b, c] {
            document.append_child(root, node);
        }
        document.detach(b);

        assert_eq!(document.parent(b), None);
        assert_eq!(document.next_sibling(a), Some(c));
        assert_eq!(document.prev_sibling(c), Some(a));
        assert_eq!(document.first_child(root), Some(a));
        assert_eq!(document.last_child(root), Some(c));
        assert_eq!(document.children(root).count(), 2);

        document.detach(a);
        document.detach(c);
        assert_eq!(document.first_child(root), None);
        assert_eq!(document.last_child(root), None);
    }

    #[test]
    fn text_content_concatenates_descendants_only() {
        let mut document = Document::new();
        let root = document.root();
        let div = document.create_element("div");
        let text = document.create_text("hello ");
        let span = document.create_element("span");
        let nested = document.create_text("world");
        let comment = document.create_comment("ignored");
        document.append_child(root, div);
        document.append_child(div, text);
        document.append_child(div, span);
        document.append_child(div, comment);
        document.append_child(span, nested);

        assert_eq!(document.text_content(div), "hello world");
        assert_eq!(document.text_content(text), "hello ");
        assert_eq!(document.text_content(comment), "ignored");
        assert_eq!(text_of(&document, nested).as_deref(), Some("world"));
    }

    #[test]
    fn descendants_do_not_escape_their_subtree() {
        let mut document = Document::new();
        let root = document.root();
        let outer = document.create_element("outer");
        let inner = document.create_element("inner");
        let leaf = document.create_text("leaf");
        let sibling = document.create_element("sibling");
        document.append_child(root, outer);
        document.append_child(root, sibling);
        document.append_child(outer, inner);
        document.append_child(inner, leaf);

        let seen: Vec<_> = document.descendants(inner).collect();
        assert_eq!(seen, vec![inner, leaf]);
        let all: Vec<_> = document.descendants(root).collect();
        assert_eq!(all, vec![root, outer, inner, leaf, sibling]);
    }

    #[test]
    fn attributes_can_be_set_removed_and_read() {
        let mut document = Document::new();
        let mut attributes = Attributes::new();
        attributes.insert("class", "hero");
        let div = document.create_element_with_attributes("DIV", attributes);
        assert_eq!(document.get_attribute(div, "class"), Some("hero"));
        assert_eq!(
            document.element(div).map(ElementData::name),
            Some("div"),
            "create_element must lowercase like the DOM does"
        );
        document.set_attribute(div, "id", "main");
        assert_eq!(document.get_attribute(div, "id"), Some("main"));
        assert_eq!(
            document.remove_attribute(div, "id"),
            Some("main".to_string())
        );
        assert_eq!(document.remove_attribute(div, "id"), None);
        assert_eq!(document.get_attribute(div, "id"), None);
    }

    #[test]
    fn add_attributes_if_missing_keeps_existing_values() {
        let mut document = Document::new();
        let mut attributes = Attributes::new();
        attributes.insert("id", "original");
        let div = document.create_element_with_attributes("div", attributes);
        let mut extra = Attributes::new();
        extra.insert("id", "replacement");
        extra.insert("lang", "cs");
        document.add_attributes_if_missing(div, &extra);
        assert_eq!(document.get_attribute(div, "id"), Some("original"));
        assert_eq!(document.get_attribute(div, "lang"), Some("cs"));
    }

    #[test]
    fn element_data_reports_the_html_namespace() {
        let mut document = Document::new();
        let div = document.create_element("div");
        let element = document.element(div).expect("element");
        assert_eq!(element.namespace(), HTML_NAMESPACE);
        assert!(element.is_html());
        assert!(element.attributes().is_empty());
        assert_eq!(element.template_contents(), None);
    }

    #[test]
    fn append_text_merges_adjacent_text_nodes() {
        let mut document = Document::new();
        let root = document.root();
        let div = document.create_element("div");
        document.append_child(root, div);
        document.append_text(div, "a");
        document.append_text(div, "b");
        assert_eq!(document.children(div).count(), 1);
        assert_eq!(document.text_content(div), "ab");
    }

    #[test]
    fn reparent_children_moves_the_whole_list_in_order() {
        let mut document = Document::new();
        let root = document.root();
        let from = document.create_element("from");
        let to = document.create_element("to");
        let first = document.create_element("first");
        let second = document.create_element("second");
        document.append_child(root, from);
        document.append_child(root, to);
        document.append_child(from, first);
        document.append_child(from, second);
        document.reparent_children(from, to);
        let moved: Vec<_> = document.children(to).collect();
        assert_eq!(moved, vec![first, second]);
        assert_eq!(document.children(from).count(), 0);
        assert_eq!(document.parent(first), Some(to));
    }

    #[test]
    #[should_panic(expected = "cannot insert a node into its own subtree")]
    fn append_child_rejects_a_cycle() {
        let mut document = Document::new();
        let root = document.root();
        let outer = document.create_element("outer");
        let inner = document.create_element("inner");
        document.append_child(root, outer);
        document.append_child(outer, inner);
        document.append_child(inner, outer);
    }

    #[test]
    #[should_panic(expected = "stale NodeId")]
    fn stale_handles_are_rejected() {
        let mut document = Document::new();
        let root = document.root();
        let child = document.create_element("child");
        document.append_child(root, child);
        document.remove_node(child);
        let replacement = document.create_element("replacement");
        assert_eq!(replacement.index(), child.index());
        let _ = document.node(child);
    }

    #[test]
    fn remove_node_frees_the_subtree_and_repairs_the_parent() {
        let mut document = Document::new();
        let root = document.root();
        let outer = document.create_element("outer");
        let inner = document.create_element("inner");
        let leaf = document.create_text("leaf");
        let survivor = document.create_element("survivor");
        document.append_child(root, outer);
        document.append_child(root, survivor);
        document.append_child(outer, inner);
        document.append_child(inner, leaf);
        let before = document.node_count();

        assert_eq!(document.remove_node(outer), 3);
        assert_eq!(document.node_count(), before - 3);
        assert!(document.try_node(outer).is_none());
        assert!(document.try_node(leaf).is_none());
        assert_eq!(document.first_child(root), Some(survivor));
        assert_eq!(document.last_child(root), Some(survivor));
        assert_eq!(document.prev_sibling(survivor), None);
    }

    #[test]
    fn remove_node_reclaims_nested_template_fragments() {
        let mut document = Document::new();
        let root = document.root();
        let outer = document.create_element("template");
        document.append_child(root, outer);
        let outer_fragment = document
            .element(outer)
            .and_then(ElementData::template_contents)
            .expect("outer fragment");
        let inner_template = document.create_element("template");
        document.append_child(outer_fragment, inner_template);
        let inner_fragment = document
            .element(inner_template)
            .and_then(ElementData::template_contents)
            .expect("inner fragment");
        let leaf = document.create_text("deep");
        document.append_child(inner_fragment, leaf);
        assert_eq!(document.node_count(), 6);

        // root + outer + outer fragment + inner template + inner fragment + leaf.
        assert_eq!(document.remove_node(outer), 5);
        assert_eq!(document.node_count(), 1);
        assert!(document.try_node(outer_fragment).is_none());
        assert!(document.try_node(leaf).is_none());
    }

    #[test]
    fn template_contents_form_a_detached_fragment() {
        let mut document = Document::new();
        let root = document.root();
        let template = document.create_element("template");
        document.append_child(root, template);
        let fragment = document
            .element(template)
            .and_then(ElementData::template_contents)
            .expect("template must own a fragment");
        assert_eq!(document.children(template).count(), 0);
        assert_eq!(document.parent(fragment), None);
        assert!(matches!(document.kind(fragment), NodeKind::Document));
        assert!(!document.descendants(root).any(|node| node == fragment));

        // A non-template element owns no fragment.
        let div = document.create_element("div");
        assert_eq!(
            document
                .element(div)
                .and_then(ElementData::template_contents),
            None
        );
        // A `<template>` outside the HTML namespace is not an HTML template.
        let foreign_template = document.create_element_qualified(
            QualName::new(None, "http://www.w3.org/2000/svg".into(), "template".into()),
            Attributes::new(),
            false,
        );
        assert_eq!(
            document
                .element(foreign_template)
                .and_then(ElementData::template_contents),
            None
        );
    }
}
