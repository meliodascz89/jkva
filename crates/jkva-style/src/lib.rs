//! `jkva-style` — CSS parsing, selector matching, specificity and the cascade.
//!
//! This crate owns the style pipeline described in
//! [`docs/src/en/architecture.md`](https://github.com/jkva/jkva/blob/main/docs/src/en/architecture.md):
//! it turns stylesheet text into a structured model, matches selectors against a
//! [`jkva_dom::Document`], and computes a [`ComputedStyle`] for every element.
//! Per ADR-013 the *syntax* comes from `cssparser` while the cascade and the
//! computed values are ours; per ADR-014 the CSS scope is the full modern
//! language.
//!
//! # Pipeline
//!
//! ```text
//! text  ──►  Stylesheet { rules }              parse     (cssparser)
//!            ├── Rule { selectors, declarations }
//!            └── AtRule { name, prelude }       recorded, then dropped
//!
//! tree  ──►  matches(&Document, NodeId, &Selector) -> bool     matching
//!
//! sheets + tree  ──►  StyleEngine::compute_styles -> ComputedStyleMap
//!                      sorted by (important, specificity, source order),
//!                      shorthands expanded, inheritance applied
//! ```
//!
//! * [`Stylesheet::parse`] produces [`Rule`]s and [`AtRule`]s in source order and
//!   reports everything it dropped as a [`ParseWarning`].
//! * [`Declaration`] keeps `name` / `value` / `important` as raw data. Values are
//!   not validated or typed.
//! * [`Selector`] is a structural selector model with [`Specificity`]
//!   computation; [`parse_selector_list`] exposes the selector parser on its own.
//! * [`matches`] evaluates a selector against a `jkva-dom` element.
//! * [`StyleEngine`] computes the first cascade.
//!
//! # Example
//!
//! ```
//! use jkva_dom::parse_document;
//! use jkva_style::{DisplayType, StyleEngine, Stylesheet};
//!
//! let document = parse_document("<div id=app><span>hi</span></div>");
//! let mut engine = StyleEngine::new();
//! engine.add_stylesheet(
//!     Stylesheet::parse("#app { display: block; margin: 1px 2px; color: navy }").unwrap(),
//! );
//!
//! let styles = engine.compute_styles(&document);
//! let app = document
//!     .descendants(document.root())
//!     .find(|node| {
//!         document
//!             .element(*node)
//!             .is_some_and(|element| element.attribute("id") == Some("app"))
//!     })
//!     .expect("the document has #app");
//!
//! let style = styles.get(app).unwrap();
//! assert_eq!(style.display, DisplayType::Block);
//! assert_eq!(style.margin_top, "1px");
//! assert_eq!(style.margin_left, "2px");
//! // `color` is inherited, so the child picks it up without a rule of its own.
//! let span = document.first_child(app).unwrap();
//! assert_eq!(styles.get(span).unwrap().color, "navy");
//! ```
//!
//! # Deliberately not implemented yet
//!
//! ## Values
//!
//! * Computed values are cascaded *specified-value text*: `width: 50%` stays
//!   `"50%"`, `color: red` stays `"red"`, and no length is converted to pixels.
//!   Only `display` is parsed into a typed [`DisplayType`]. This is documented at
//!   [`ComputedStyle`].
//! * `var()` is **not resolved**. A declaration such as `color: var(--brand)`
//!   computes to the literal text `var(--brand)`; custom properties are parsed as
//!   ordinary declarations.
//! * Only the properties listed on [`ComputedStyle`] are tracked. `background`,
//!   `font`, `border` and every other shorthand or property is ignored — so
//!   `background: red` does **not** set `background-color`, and shorthand
//!   expansion exists only for `margin` and `padding`.
//! * There is no user-agent stylesheet: a `<div>` computes to
//!   `display: inline`, not `block`.
//!
//! ## Selectors
//!
//! * Namespace prefixes (`svg|rect`, `*|div`) never match; `@namespace` is not
//!   implemented.
//! * [`matches`] supports universal, type, ID, class and attribute selectors,
//!   `:is()`, `:where()`, `:not()`, `:root`, `:first-child`, `:last-child` and
//!   `:nth-child()`, plus all four combinators. Anything else — pseudo-elements,
//!   `:has()`, `:hover`, `:nth-last-child()`, … — never matches.
//! * No shadow DOM, no slots, no `:scope`.
//!
//! ## At-rules
//!
//! * `@media`, `@supports`, `@container`, `@layer`, `@font-face`, `@keyframes`,
//!   `@import` and every other at-rule are recorded by name and prelude and then
//!   dropped **including their contents**. A rule inside `@media` is invisible to
//!   the cascade.
//! * Decided shape for the implementation (not written yet): a **flat rule list
//!   with a condition stack** — [`Rule`] gains `conditions: Vec<Condition>` where
//!   an empty vector means "unconditional", nested `@media` blocks flatten into a
//!   conjunction, and source order plus specificity stay linear so the cascade
//!   remains one ordered walk.
//! * No CSS Nesting: a style rule nested inside another parses as a malformed
//!   declaration and is dropped.
//!
//! ## Parsing
//!
//! * [`Stylesheet::parse`] returns `Result`, but it is **fatal only for the
//!   parser's nesting limit** ([`ErrorKind::TooDeeplyNested`], reached through
//!   deeply nested selector blocks). Every other problem is recoverable by design
//!   and reported through [`Stylesheet::warnings`]; the signature is kept so that
//!   future fatal conditions do not change it.
//! * `:nth-child(… of S)` is stored opaquely, so the specificity of `S` is not
//!   counted.
//!
//! ## Engine
//!
//! * Matching is a linear scan over all rules for every element; there is no rule
//!   index, no invalidation, no style sharing and no incremental restyle.
//! * [`ComputedStyleMap`] iteration order is unspecified.
//!
//! # Invariants
//!
//! - No `unsafe` code (ADR-005).
//! - Every public item is documented; `missing_docs` is denied.
//! - The only JKva dependency is `jkva-dom`, so the parse and match layers stay
//!   testable without the engine.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod cascade;
pub mod computed;
pub mod declaration;
pub mod error;
pub mod matching;
mod parser;
pub mod selector;
pub mod specificity;
pub mod stylesheet;

pub use crate::cascade::{ComputedStyleMap, StyleEngine};
pub use crate::computed::{ComputedStyle, DisplayType};
pub use crate::declaration::Declaration;
pub use crate::error::{ErrorKind, ParseError, ParseWarning, Position};
pub use crate::matching::{matches, matches_any};
pub use crate::selector::{
    AttributeMatcher, AttributeOperator, AttributeSelector, Combinator, CompoundSelector,
    PseudoClass, PseudoElement, Selector, SelectorComponent, SimpleSelector, parse_selector_list,
};
pub use crate::specificity::Specificity;
pub use crate::stylesheet::{AtRule, Rule, Stylesheet};

/// Version of the `jkva-style` crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
