//! Node payloads: node kinds, element data, attributes and the small enums.
//!
//! A node stores nothing but its [`NodeKind`] and the five links of the tree
//! (`parent`, `first_child`, `last_child`, `prev_sibling`, `next_sibling`). The
//! links are private: every operation that changes them lives on
//! [`Document`](crate::Document), which keeps them consistent. Readers use the
//! accessors below.

use html5ever::QualName;
use jkva_core::Id;

/// Namespace URI of ordinary HTML elements.
pub const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";

/// Opaque handle to a node stored in a [`Document`](crate::Document) arena.
///
/// The handle is generation-checked by [`jkva_core::Arena`]: a node that has been
/// removed never resolves again, even when its arena slot is reused. Holding a
/// `NodeId` therefore proves nothing about liveness; resolve it through
/// [`Document::node`](crate::Document::node) or
/// [`Document::try_node`](crate::Document::try_node).
///
/// A `NodeId` is not bound to one document: mixing handles between two documents
/// is not detected and can resolve to an unrelated node. Keep handles inside the
/// document that issued them.
pub type NodeId = Id<NodeData>;

/// What a node is.
///
/// The payload is kept out of line from the links in [`NodeData`] so that
/// traversal never touches text or attribute storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// The document root, and also a `<template>`'s contents fragment.
    ///
    /// A fragment is a detached root: it has no parent, it is not reachable from
    /// [`Document::root`](crate::Document::root), and it is owned by its
    /// `<template>` element ([`ElementData::template_contents`]).
    Document,
    /// An element with a qualified name, attributes and parse-time flags.
    Element(ElementData),
    /// Character data, already entity-decoded by the parser.
    ///
    /// The parser never produces two adjacent text nodes; the tree builder merges
    /// them, as the HTML specification requires.
    Text(String),
    /// A comment, holding the text between `<!--` and `-->`.
    Comment(String),
}

/// One node of the tree: its payload plus the links that place it in the tree.
///
/// Created by [`Document`](crate::Document) only; the links are maintained
/// exclusively by the document, so a node can never be half-attached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeData {
    kind: NodeKind,
    parent: Option<NodeId>,
    first_child: Option<NodeId>,
    last_child: Option<NodeId>,
    prev_sibling: Option<NodeId>,
    next_sibling: Option<NodeId>,
}

impl NodeData {
    /// Creates a detached node with the given payload.
    pub(crate) const fn new(kind: NodeKind) -> Self {
        Self {
            kind,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
        }
    }

    /// Returns the payload of this node.
    #[must_use]
    pub const fn kind(&self) -> &NodeKind {
        &self.kind
    }

    /// Returns the payload of this node for mutation.
    pub(crate) const fn kind_mut(&mut self) -> &mut NodeKind {
        &mut self.kind
    }

    /// Returns the parent node, or `None` for the document root and for
    /// detached fragments and nodes.
    #[must_use]
    pub const fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    /// Returns the first child in tree order.
    #[must_use]
    pub const fn first_child(&self) -> Option<NodeId> {
        self.first_child
    }

    /// Returns the last child in tree order.
    #[must_use]
    pub const fn last_child(&self) -> Option<NodeId> {
        self.last_child
    }

    /// Returns the previous sibling in tree order.
    #[must_use]
    pub const fn prev_sibling(&self) -> Option<NodeId> {
        self.prev_sibling
    }

    /// Returns the next sibling in tree order.
    #[must_use]
    pub const fn next_sibling(&self) -> Option<NodeId> {
        self.next_sibling
    }

    /// Returns `true` when this node is an element.
    #[must_use]
    pub const fn is_element(&self) -> bool {
        matches!(self.kind, NodeKind::Element(_))
    }

    /// Returns the element data of an element node, and `None` for every other
    /// kind of node.
    #[must_use]
    pub const fn as_element(&self) -> Option<&ElementData> {
        match &self.kind {
            NodeKind::Element(element) => Some(element),
            _ => None,
        }
    }

    /// Returns the character data of a text node.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Sets the parent link.
    pub(crate) const fn set_parent(&mut self, parent: Option<NodeId>) {
        self.parent = parent;
    }

    /// Sets the first-child link.
    pub(crate) const fn set_first_child(&mut self, child: Option<NodeId>) {
        self.first_child = child;
    }

    /// Sets the last-child link.
    pub(crate) const fn set_last_child(&mut self, child: Option<NodeId>) {
        self.last_child = child;
    }

    /// Sets the previous-sibling link.
    pub(crate) const fn set_prev_sibling(&mut self, sibling: Option<NodeId>) {
        self.prev_sibling = sibling;
    }

    /// Sets the next-sibling link.
    pub(crate) const fn set_next_sibling(&mut self, sibling: Option<NodeId>) {
        self.next_sibling = sibling;
    }
}

/// An element: its qualified name, its attributes and the parse-time flags the
/// HTML tree builder asked us to record.
///
/// The qualified name keeps the namespace that the parser resolved, so elements
/// created by the foreign-content rules (`<svg>`, `<math>`) can be told apart
/// from HTML elements later. [`Self::name`] exposes only the local part; use
/// [`Self::namespace`] for the namespace URI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementData {
    qual_name: QualName,
    attributes: Attributes,
    template_contents: Option<NodeId>,
    mathml_annotation_xml_integration_point: bool,
}

impl ElementData {
    /// Creates element data from a parsed qualified name.
    ///
    /// `template_contents` must be the detached fragment that receives the
    /// children of a `<template>` element; it is `None` for every other element.
    pub(crate) const fn new(
        qual_name: QualName,
        attributes: Attributes,
        template_contents: Option<NodeId>,
        mathml_annotation_xml_integration_point: bool,
    ) -> Self {
        Self {
            qual_name,
            attributes,
            template_contents,
            mathml_annotation_xml_integration_point,
        }
    }

    /// Returns the local name of the element, for example `div`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.qual_name.local
    }

    /// Returns the namespace URI of the element.
    ///
    /// This is [`HTML_NAMESPACE`] for ordinary HTML elements, the SVG or MathML
    /// namespace for foreign content, and the empty string for the null
    /// namespace.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.qual_name.ns
    }

    /// Returns `true` when the element is in the HTML namespace.
    #[must_use]
    pub fn is_html(&self) -> bool {
        &*self.qual_name.ns == HTML_NAMESPACE
    }

    /// Returns the attributes of the element, in source order.
    #[must_use]
    pub const fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    /// Returns the value of one attribute, or `None` when it is absent.
    ///
    /// The lookup is an exact byte comparison on the attribute name. That matches
    /// the HTML parser, which lowercases HTML attribute names before they reach
    /// the tree.
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.get(name)
    }

    /// Returns the detached fragment that holds the children of a `<template>`
    /// element, or `None` for every other element.
    ///
    /// The fragment is not a child of the template: per the HTML specification the
    /// template element has no child nodes and its contents live in a separate
    /// document fragment.
    #[must_use]
    pub const fn template_contents(&self) -> Option<NodeId> {
        self.template_contents
    }

    /// Returns `true` when the tree builder flagged this element as a MathML
    /// `annotation-xml` HTML integration point.
    #[must_use]
    pub const fn is_mathml_annotation_xml_integration_point(&self) -> bool {
        self.mathml_annotation_xml_integration_point
    }

    /// Returns the qualified name with its namespace.
    pub(crate) const fn qual_name(&self) -> &QualName {
        &self.qual_name
    }

    /// Returns the attribute map for mutation.
    pub(crate) const fn attributes_mut(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}

/// The attributes of one element, in source order with unique names.
///
/// The map is a small ordered vector, not a hash map: real elements carry a
/// handful of attributes, and iterating them in source order is what the parser,
/// serialisation and tests need. Inserting a name that already exists replaces
/// the value in place and keeps the original position.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attributes {
    entries: Vec<(String, String)>,
}

impl Attributes {
    /// Creates an empty attribute map.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Returns the value stored under `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.0 == name)
            .map(|entry| entry.1.as_str())
    }

    /// Returns `true` when `name` is present.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Inserts or replaces an attribute and returns the previous value.
    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>) -> Option<String> {
        let name = name.into();
        let value = value.into();
        match self.entries.iter().position(|entry| entry.0 == name) {
            Some(index) => Some(std::mem::replace(&mut self.entries[index].1, value)),
            None => {
                self.entries.push((name, value));
                None
            }
        }
    }

    /// Inserts an attribute only when the name is not present yet.
    ///
    /// The HTML tree builder needs this when it re-opens an element and must not
    /// overwrite attributes that are already set.
    pub(crate) fn insert_if_missing(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        if !self.contains(&name) {
            self.entries.push((name, value.into()));
        }
    }

    /// Removes an attribute and returns its value.
    pub fn remove(&mut self, name: &str) -> Option<String> {
        let index = self.entries.iter().position(|entry| entry.0 == name)?;
        Some(self.entries.remove(index).1)
    }

    /// Iterates over the attributes in source order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.entries
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// Returns the number of attributes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when the element has no attributes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The `<!DOCTYPE>` of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Doctype {
    name: String,
    public_id: String,
    system_id: String,
}

impl Doctype {
    /// Creates a doctype from the three identifiers the tokenizer produced.
    pub(crate) const fn new(name: String, public_id: String, system_id: String) -> Self {
        Self {
            name,
            public_id,
            system_id,
        }
    }

    /// Returns the doctype name, for example `html`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the public identifier, or the empty string when absent.
    #[must_use]
    pub fn public_id(&self) -> &str {
        &self.public_id
    }

    /// Returns the system identifier, or the empty string when absent.
    #[must_use]
    pub fn system_id(&self) -> &str {
        &self.system_id
    }
}

/// The document's quirks mode, as computed by the HTML tree builder.
///
/// Quirks mode is set from the doctype and changes how legacy documents are laid
/// out; [`Document::quirks_mode`](crate::Document::quirks_mode) reports it. A
/// document without a doctype is in [`QuirksMode::Quirks`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum QuirksMode {
    /// Standards mode.
    #[default]
    NoQuirks,
    /// Almost standards mode.
    LimitedQuirks,
    /// Full quirks mode.
    Quirks,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_keep_source_order_and_unique_names() {
        let mut attributes = Attributes::new();
        assert!(attributes.is_empty());
        assert_eq!(attributes.insert("id", "a"), None);
        assert_eq!(attributes.insert("class", "b"), None);
        assert_eq!(attributes.insert("id", "c"), Some("a".to_string()));
        assert_eq!(attributes.len(), 2);
        let seen: Vec<_> = attributes.iter().collect();
        assert_eq!(seen, vec![("id", "c"), ("class", "b")]);
    }

    #[test]
    fn attributes_lookup_and_removal() {
        let mut attributes = Attributes::new();
        attributes.insert("href", "/index.html");
        assert_eq!(attributes.get("href"), Some("/index.html"));
        assert_eq!(attributes.get("HREF"), None);
        assert!(attributes.contains("href"));
        assert_eq!(attributes.remove("href"), Some("/index.html".to_string()));
        assert_eq!(attributes.remove("href"), None);
        assert!(!attributes.contains("href"));
    }

    #[test]
    fn insert_if_missing_never_overwrites() {
        let mut attributes = Attributes::new();
        attributes.insert("id", "kept");
        attributes.insert_if_missing("id", "dropped");
        attributes.insert_if_missing("lang", "cs");
        assert_eq!(attributes.get("id"), Some("kept"));
        assert_eq!(attributes.get("lang"), Some("cs"));
    }

    #[test]
    fn quirks_mode_defaults_to_standards() {
        assert_eq!(QuirksMode::default(), QuirksMode::NoQuirks);
    }

    #[test]
    fn node_data_starts_detached() {
        let node = NodeData::new(NodeKind::Text("x".into()));
        assert_eq!(node.parent(), None);
        assert_eq!(node.first_child(), None);
        assert_eq!(node.last_child(), None);
        assert_eq!(node.prev_sibling(), None);
        assert_eq!(node.next_sibling(), None);
        assert_eq!(node.text(), Some("x"));
        assert!(!node.is_element());
        assert!(node.as_element().is_none());
    }
}
