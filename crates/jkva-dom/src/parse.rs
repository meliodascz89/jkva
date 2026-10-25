//! HTML parsing: an [`html5ever`] tree sink that builds a [`Document`].
//!
//! The parser writes through `&self` handles, so the document sits in a
//! [`RefCell`] for the duration of the parse and is moved out untouched in
//! [`TreeSink::finish`]. Nothing borrows across a call into the tree builder, so
//! the `RefCell` is never re-entered.

use std::borrow::Cow;
use std::cell::{Ref, RefCell};

use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode as HtmlQuirksMode, TreeSink};
use html5ever::{Attribute, ParseOpts, QualName};

use crate::document::Document;
use crate::node::{Attributes, Doctype, ElementData, NodeData, NodeId, QuirksMode};

/// Parses a complete HTML document into a [`Document`] tree.
///
/// The function implements the HTML parsing algorithm through
/// [`html5ever`](https://docs.rs/html5ever) `0.40`, so implied `<html>`, `<head>`
/// and `<body>` elements, entity decoding, foster parenting, adoption agency and
/// error recovery all match the specification. Parser diagnostics end up in
/// [`Document::parse_errors`], the doctype in [`Document::doctype`] and the
/// quirks mode in [`Document::quirks_mode`].
///
/// `<template>` elements get a detached contents fragment, reported by
/// [`ElementData::template_contents`]; it is not a child of the element and is
/// not reachable from [`Document::root`].
///
/// # Examples
///
/// ```
/// let document = jkva_dom::parse_document("<p>Hello</p>");
/// let root = document.root();
/// let html = document.first_child(root).expect("implied <html>");
/// assert!(document.text_content(html).contains("Hello"));
/// ```
#[must_use]
pub fn parse_document(html: &str) -> Document {
    let sink = HtmlSink::new();
    html5ever::parse_document(sink, ParseOpts::default()).one(html)
}

/// The tree sink that turns html5ever callbacks into [`Document`] mutations.
struct HtmlSink {
    document: RefCell<Document>,
}

impl HtmlSink {
    /// Creates a sink over a fresh, empty document.
    fn new() -> Self {
        Self {
            document: RefCell::new(Document::new()),
        }
    }
}

impl TreeSink for HtmlSink {
    type Handle = NodeId;
    type Output = Document;
    type ElemName<'a> = Ref<'a, QualName>;

    fn finish(self) -> Document {
        self.document.into_inner()
    }

    fn parse_error(&self, msg: Cow<'static, str>) {
        self.document
            .borrow_mut()
            .record_parse_error(msg.into_owned());
    }

    fn get_document(&self) -> NodeId {
        self.document.borrow().root()
    }

    fn elem_name<'a>(&'a self, target: &'a NodeId) -> Ref<'a, QualName> {
        Ref::map(self.document.borrow(), |document| {
            document.element_data(*target).qual_name()
        })
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> NodeId {
        let attributes = collect_attributes(attrs);
        // `Document` derives the template rule from the qualified name itself, so
        // `flags.template` needs no separate plumbing.
        self.document.borrow_mut().create_element_qualified(
            name,
            attributes,
            flags.mathml_annotation_xml_integration_point,
        )
    }

    fn create_comment(&self, text: StrTendril) -> NodeId {
        self.document.borrow_mut().create_comment(&text)
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> NodeId {
        // The HTML parsing algorithm never produces a processing instruction: it
        // tokenises `<?...>` as a bogus comment. Keeping that text in a comment
        // node preserves the input without inventing a node kind.
        self.document
            .borrow_mut()
            .create_comment(&format!("?{} {}", &*target, &*data))
    }

    fn append(&self, parent: &NodeId, child: NodeOrText<NodeId>) {
        let mut document = self.document.borrow_mut();
        match child {
            NodeOrText::AppendNode(node) => document.append_moving(*parent, node),
            NodeOrText::AppendText(text) => document.append_text(*parent, &text),
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &NodeId,
        prev_element: &NodeId,
        child: NodeOrText<NodeId>,
    ) {
        let mut document = self.document.borrow_mut();
        let attached = document
            .try_node(*element)
            .and_then(NodeData::parent)
            .is_some();
        match child {
            NodeOrText::AppendNode(node) => {
                if attached {
                    document.insert_before_moving(*element, node);
                } else {
                    document.append_moving(*prev_element, node);
                }
            }
            NodeOrText::AppendText(text) => {
                if attached {
                    document.insert_text_before(*element, &text);
                } else {
                    document.append_text(*prev_element, &text);
                }
            }
        }
    }

    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        let doctype = Doctype::new(
            name.to_string(),
            public_id.to_string(),
            system_id.to_string(),
        );
        self.document.borrow_mut().set_doctype(doctype);
    }

    fn get_template_contents(&self, target: &NodeId) -> NodeId {
        self.document
            .borrow()
            .element(*target)
            .and_then(ElementData::template_contents)
            .unwrap_or_else(|| panic!("template contents requested for a non-template node"))
    }

    fn same_node(&self, x: &NodeId, y: &NodeId) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, mode: HtmlQuirksMode) {
        let mode = match mode {
            HtmlQuirksMode::NoQuirks => QuirksMode::NoQuirks,
            HtmlQuirksMode::LimitedQuirks => QuirksMode::LimitedQuirks,
            HtmlQuirksMode::Quirks => QuirksMode::Quirks,
        };
        self.document.borrow_mut().set_quirks_mode(mode);
    }

    fn append_before_sibling(&self, sibling: &NodeId, new_node: NodeOrText<NodeId>) {
        let mut document = self.document.borrow_mut();
        match new_node {
            NodeOrText::AppendNode(node) => document.insert_before_moving(*sibling, node),
            NodeOrText::AppendText(text) => document.insert_text_before(*sibling, &text),
        }
    }

    fn add_attrs_if_missing(&self, target: &NodeId, attrs: Vec<Attribute>) {
        let attributes = collect_attributes(attrs);
        self.document
            .borrow_mut()
            .add_attributes_if_missing(*target, &attributes);
    }

    fn remove_from_parent(&self, target: &NodeId) {
        self.document.borrow_mut().detach(*target);
    }

    fn reparent_children(&self, node: &NodeId, new_parent: &NodeId) {
        self.document
            .borrow_mut()
            .reparent_children(*node, *new_parent);
    }

    fn is_mathml_annotation_xml_integration_point(&self, handle: &NodeId) -> bool {
        self.document
            .borrow()
            .element(*handle)
            .is_some_and(ElementData::is_mathml_annotation_xml_integration_point)
    }

    fn allow_declarative_shadow_roots(&self, _intended_parent: &NodeId) -> bool {
        // Declarative shadow DOM is on the ADR-014 roadmap but not implemented in
        // this version. Returning `false` keeps `<template shadowrootmode>` as an
        // ordinary template element instead of silently attaching a shadow root
        // that we cannot represent yet.
        false
    }
}

/// Copies parsed attributes into our attribute map, keeping source order.
///
/// Attribute names arrive already lowercased for HTML. Foreign content keeps its
/// prefix, so `xlink:href` becomes `"xlink:href"`.
fn collect_attributes(attrs: Vec<Attribute>) -> Attributes {
    let mut attributes = Attributes::new();
    for attr in attrs {
        let name = match &attr.name.prefix {
            Some(prefix) => format!("{prefix}:{}", attr.name.local),
            None => attr.name.local.to_string(),
        };
        attributes.insert(name, attr.value.to_string());
    }
    attributes
}
