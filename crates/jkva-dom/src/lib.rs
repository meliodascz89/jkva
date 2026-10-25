//! `jkva-dom` — the HTML document tree of the JKva engine.
//!
//! See `docs/` for the architecture and `briefing/DECISIONS.md` for the decisions
//! this crate must follow.
//!
//! The tree is an arena of nodes with generation-checked handles (ADR-008):
//! [`Document`] owns one [`jkva_core::Arena`], a [`NodeId`] is an index plus a
//! generation, and dropping the document frees the whole tree at once. There are
//! no `Rc` cycles and no `unsafe` code in this crate.
//!
//! # Layout of a node
//!
//! A [`NodeData`] is a [`NodeKind`] payload plus five links: parent, first child,
//! last child, previous sibling and next sibling. The links are private and are
//! only written by [`Document`], so a reader can never observe a node that is
//! half-attached. Reading goes through [`Document::parent`],
//! [`Document::children`], [`Document::descendants`] and friends.
//!
//! # Parsing
//!
//! [`parse_document`] runs the HTML parsing algorithm from
//! [`html5ever`](https://docs.rs/html5ever) `0.40` and writes the result through
//! `html5ever`'s `TreeSink` interface (ADR-008, ADR-013). The sink is private:
//! callers get a finished [`Document`], not a parser handle. Fragments
//! (`innerHTML`-style parsing) are not exposed yet.
//!
//! ```
//! use jkva_dom::{NodeKind, parse_document};
//!
//! let document = parse_document("<!DOCTYPE html><title>t</title><p>Hello");
//! let html = document
//!     .first_child(document.root())
//!     .expect("the parser implies <html>");
//! let head = document.first_child(html).expect("implied <head>");
//! let body = document.next_sibling(head).expect("implied <body>");
//! assert_eq!(document.text_content(body), "Hello");
//! assert!(matches!(document.kind(html), NodeKind::Element(_)));
//! assert_eq!(document.doctype().map(|doctype| doctype.name()), Some("html"));
//! ```
//!
//! # What this crate does not do (yet)
//!
//! - No events, no `NodeList`/`HTMLCollection` wrappers, no shadow roots: those
//!   arrive with the DOM API layer (`jkva-api`) and the ADR-014 work.
//! - No mutation observer, no attribute-change notifications. Styling will be
//!   invalidated by whoever calls [`Document::set_attribute`], not by the tree.
//! - No fragment parsing and no serialisation.
//! - Namespace prefixes are kept on attributes, but [`ElementData::name`] exposes
//!   the local name only; matching by namespace is up to `jkva-style`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod document;
mod node;
mod parse;

pub use document::{Children, Descendants, Document, MAX_RETAINED_PARSE_ERRORS, ParseErrors};
pub use node::{
    Attributes, Doctype, ElementData, HTML_NAMESPACE, NodeData, NodeId, NodeKind, QuirksMode,
};
pub use parse::parse_document;

/// Version of the JKva DOM crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
