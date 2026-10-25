//! `jkva-style` — CSS parsing, the rule data model and selector specificity.
//!
//! This crate owns the first stage of the style pipeline described in
//! [`docs/src/en/architecture.md`](https://github.com/jkva/jkva/blob/main/docs/src/en/architecture.md):
//! it turns stylesheet text into a structured, order-preserving model that the
//! cascade can consume later. Per ADR-013 the *syntax* comes from `cssparser`
//! and the cascade and computed values will be ours; per ADR-014 the CSS scope
//! is the full modern language.
//!
//! # Scope of this crate today
//!
//! ```text
//! text ──► Stylesheet { rules: [Rule { selectors, declarations }], at_rules }
//! ```
//!
//! * [`Stylesheet::parse`] turns CSS source into [`Rule`]s and [`AtRule`]s,
//!   preserving source order and reporting everything it had to drop as a
//!   [`ParseWarning`].
//! * [`Declaration`] keeps `name` / `value` / `important` as raw data. Values
//!   are not validated, typed or normalised.
//! * [`Selector`] is a structural selector model with [`Specificity`]
//!   computation.
//! * [`parse_selector_list`] exposes the selector parser on its own, which is
//!   what `querySelector()` and friends will need.
//!
//! # Deliberately not implemented yet
//!
//! This crate is decoupled from `jkva-dom` on purpose, so that the cascade can
//! be added without a rewrite. As a consequence:
//!
//! * **No cascade, matching or computed values.** No `declarations_for()`, no
//!   inheritance, no `var()` resolution, no unit conversion. `!important` is
//!   recorded but has no effect.
//! * **At-rules are skipped, not applied.** `@media`, `@supports`, `@container`,
//!   `@scope`, `@layer`, `@font-face`, `@keyframes`, `@import` and every other
//!   at-rule are recorded by name and prelude and then dropped *including their
//!   contents*. Rules inside an `@media` block are therefore invisible to later
//!   stages.
//! * **No CSS Nesting.** A style rule nested inside another one parses as a
//!   malformed declaration and is dropped.
//! * **No custom property semantics.** `--x: 1px` is stored like any other
//!   declaration, `var()` is not resolved, and a top-level `!` inside a custom
//!   property value is treated as an (invalid) `!important`.
//! * **Namespaces are not resolved.** `@namespace` is not supported; namespace
//!   prefixes such as `svg|rect` are preserved verbatim for the matcher.
//! * **`:nth-child(… of S)` is not parsed.** The argument stays opaque, so the
//!   specificity of `S` is not counted.
//! * **No selector matching.** [`Selector`] describes a selector; nothing here
//!   evaluates one against an element.
//!
//! # Example
//!
//! ```
//! use jkva_style::{Specificity, Stylesheet};
//!
//! let sheet = Stylesheet::parse(
//!     r#"
//!     /* A comment, ignored. */
//!     #hero > .title { color: red !important }
//!     @media (min-width: 600px) { p { color: blue } }
//!     p { margin: 0   auto }
//!     "#,
//! )
//! .unwrap();
//!
//! assert_eq!(sheet.rules().len(), 2);
//! assert_eq!(sheet.at_rules().len(), 1);
//! assert_eq!(sheet.at_rules()[0].name, "media");
//!
//! let rule = &sheet.rules()[0];
//! assert_eq!(rule.selectors[0].to_string(), "#hero > .title");
//! assert_eq!(rule.selectors[0].specificity(), Specificity::new(1, 1, 0));
//! assert!(rule.declarations[0].important);
//! ```
//!
//! # Invariants
//!
//! - No `unsafe` code (ADR-005).
//! - Every public item is documented; `missing_docs` is denied.
//! - The crate depends on no other JKva crate, so it can be tested and fuzzed
//!   in isolation.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod declaration;
pub mod error;
mod parser;
pub mod selector;
pub mod specificity;
pub mod stylesheet;

pub use crate::declaration::Declaration;
pub use crate::error::{ErrorKind, ParseError, ParseWarning, Position};
pub use crate::selector::{
    AttributeMatcher, AttributeOperator, AttributeSelector, Combinator, CompoundSelector,
    PseudoClass, PseudoElement, Selector, SelectorComponent, SimpleSelector, parse_selector_list,
};
pub use crate::specificity::Specificity;
pub use crate::stylesheet::{AtRule, Rule, Stylesheet};

/// Version of the `jkva-style` crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
