//! Selector matching against a real [`Document`].
//!
//! [`matches`] is the bridge between the selector model produced by
//! [`parse_selector_list`](crate::parse_selector_list) and a `jkva-dom` tree.
//!
//! # Supported selectors
//!
//! | Construct | Supported |
//! |---|---|
//! | Universal `*` | yes |
//! | Type `div` | yes, for HTML elements case-insensitively, for foreign elements case-sensitively |
//! | ID `#main` | yes |
//! | Class `.note` | yes |
//! | Attribute `[a]`, `[a=v]`, `[a~=v]`, `[a\|=v]`, `[a^=v]`, `[a$=v]`, `[a*=v]`, with the `i` flag | yes |
//! | `:is()`, `:where()`, `:not()` | yes |
//! | `:root`, `:first-child`, `:last-child`, `:nth-child(An+B)` | yes |
//! | Combinators: descendant, `>`, `+`, `~` | yes |
//! | Everything else (pseudo-elements, `:has()`, `:hover`, `:nth-last-child()`, …) | **no — never matches** |
//!
//! An unsupported construct makes the enclosing *compound selector* fail to
//! match, which [`matches`] reports as `false` rather than as an error. Inside
//! `:not()` that inverts: `:not(:hover)` matches, because `:hover` cannot be
//! evaluated while nothing is being hovered. That is the intuitive result for
//! state pseudo-classes, but it is not sound for every unsupported selector
//! (`:not(:has(img))` also matches), so do not build behaviour on it.
//!
//! # Not supported
//!
//! * Namespace prefixes: `svg|rect` and `*|div` never match, because `@namespace`
//!   is not implemented. Element and attribute names are compared by their local
//!   part.
//! * `:has()` and relative selectors: [`Selector::leading_combinator`] being set
//!   is reported as `false`.
//! * Shadow DOM, slots and custom elements (ADR-014 work, not started).
//! * `:scope`; there is no element context in this API yet.
//!
//! # Cost
//!
//! Matching is a right-to-left walk with backtracking over the ancestor and
//! sibling axes. There is no rule index and no bloom filter yet, so the caller
//! pays `documents × selectors × tree depth` per pass; the cascade documents the
//! same limitation.

use cssparser::parse_nth;
use jkva_dom::{Document, ElementData, NodeId};

use crate::selector::{
    AttributeOperator, AttributeSelector, Combinator, CompoundSelector, PseudoClass, Selector,
    SelectorComponent, SimpleSelector,
};

/// Returns `true` when `node` matches `selector`.
///
/// Use [`parse_selector_list`](crate::parse_selector_list) to turn selector text
/// into the [`Selector`] values this function takes.
///
/// Returns `false` — never panics — when `node` is stale, is not an element, or
/// when the selector uses a construct listed as unsupported in the module docs.
///
/// ```
/// use jkva_dom::parse_document;
/// use jkva_style::{matches, parse_selector_list};
///
/// let document = parse_document("<div id=app class=shell><p>hi</p></div>");
/// let div = document
///     .descendants(document.root())
///     .find(|node| document.element(*node).is_some_and(|e| e.name() == "div"))
///     .expect("the document has a div");
///
/// let selectors = parse_selector_list("#app.shell > p, div").unwrap();
/// assert!(matches(&document, div, &selectors[1]));
/// assert!(!matches(&document, div, &selectors[0]), "the div is not a p");
/// ```
pub fn matches(document: &Document, node: NodeId, selector: &Selector) -> bool {
    // A relative selector only has meaning inside `:has()`, which we do not
    // evaluate, so on its own it matches nothing.
    if selector.leading_combinator().is_some() {
        return false;
    }
    let components = selector.components();
    let Some((last, rest)) = components.split_last() else {
        return false;
    };
    matches_component(document, node, last, rest)
}

/// Returns `true` when `node` matches any of `selectors`.
pub fn matches_any(document: &Document, node: NodeId, selectors: &[Selector]) -> bool {
    selectors
        .iter()
        .any(|selector| matches(document, node, selector))
}

/// Recursively matches one component of a complex selector and every component
/// to its left.
///
/// `remaining` holds the components left of `component`, rightmost last, so the
/// walk always takes the last element.
fn matches_component(
    document: &Document,
    node: NodeId,
    component: &SelectorComponent,
    remaining: &[SelectorComponent],
) -> bool {
    let Some(element) = document.element(node) else {
        return false;
    };
    if !matches_compound(document, node, element, &component.compound) {
        return false;
    }
    let Some((previous, rest)) = remaining.split_last() else {
        return true;
    };

    match component.combinator.unwrap_or(Combinator::Descendant) {
        Combinator::Child => parent_element(document, node)
            .is_some_and(|parent| matches_component(document, parent, previous, rest)),
        Combinator::Descendant => {
            let mut ancestor = parent_element(document, node);
            while let Some(candidate) = ancestor {
                if matches_component(document, candidate, previous, rest) {
                    return true;
                }
                ancestor = parent_element(document, candidate);
            }
            false
        }
        Combinator::NextSibling => previous_element_sibling(document, node)
            .is_some_and(|sibling| matches_component(document, sibling, previous, rest)),
        Combinator::LaterSibling => {
            let mut sibling = previous_element_sibling(document, node);
            while let Some(candidate) = sibling {
                if matches_component(document, candidate, previous, rest) {
                    return true;
                }
                sibling = previous_element_sibling(document, candidate);
            }
            false
        }
    }
}

/// Returns `true` when every simple selector of the compound matches.
fn matches_compound(
    document: &Document,
    node: NodeId,
    element: &ElementData,
    compound: &CompoundSelector,
) -> bool {
    // An empty compound selector is only producible by hand; it matches nothing
    // rather than everything, so that a malformed selector cannot accidentally
    // style the whole document.
    if compound.simples.is_empty() {
        return false;
    }
    compound
        .simples
        .iter()
        .all(|simple| matches_simple(document, node, element, simple))
}

/// Returns `true` when one simple selector matches.
fn matches_simple(
    document: &Document,
    node: NodeId,
    element: &ElementData,
    simple: &SimpleSelector,
) -> bool {
    match simple {
        SimpleSelector::Universal => true,
        SimpleSelector::Type(name) => matches_type(element, name),
        SimpleSelector::Id(id) => element.attribute("id") == Some(id.as_str()),
        SimpleSelector::Class(class) => matches_class(element, class),
        SimpleSelector::Attribute(attribute) => matches_attribute(element, attribute),
        SimpleSelector::PseudoClass(pseudo_class) => {
            matches_pseudo_class(document, node, pseudo_class)
        }
        // A pseudo-element is not an element; nothing here matches it.
        SimpleSelector::PseudoElement(_) => false,
    }
}

/// Matches a type selector against the element's local name.
///
/// HTML type selectors are ASCII case-insensitive; type selectors for foreign
/// content (SVG, MathML) are case-sensitive, which matters for names such as
/// `foreignObject`.
fn matches_type(element: &ElementData, name: &str) -> bool {
    // A namespace prefix cannot be resolved without `@namespace` support.
    if name.contains('|') {
        return false;
    }
    if element.is_html() {
        element.name().eq_ignore_ascii_case(name)
    } else {
        element.name() == name
    }
}

/// Matches a class selector against the element's `class` attribute.
fn matches_class(element: &ElementData, class: &str) -> bool {
    element
        .attribute("class")
        .is_some_and(|value| value.split_ascii_whitespace().any(|item| item == class))
}

/// Looks an attribute up by name, falling back to a case-insensitive scan for
/// HTML elements, because HTML attribute names are ASCII case-insensitive.
fn attribute_value<'a>(element: &'a ElementData, name: &str) -> Option<&'a str> {
    if let Some(value) = element.attribute(name) {
        return Some(value);
    }
    if !element.is_html() {
        return None;
    }
    element
        .attributes()
        .iter()
        .find(|(attribute, _)| attribute.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
}

/// Matches an attribute selector.
fn matches_attribute(element: &ElementData, selector: &AttributeSelector) -> bool {
    // A namespace prefix cannot be resolved without `@namespace` support.
    if selector.name.contains('|') {
        return false;
    }
    let Some(actual) = attribute_value(element, &selector.name) else {
        return false;
    };
    let Some(matcher) = &selector.matcher else {
        // `[attr]`: presence only.
        return true;
    };
    let expected = matcher.value.as_str();
    let case_insensitive = selector.case_insensitive;

    match matcher.operator {
        AttributeOperator::Equals => equal(actual, expected, case_insensitive),
        AttributeOperator::Includes => {
            // The value must be a single word; an empty or multi-word value can
            // never be a member of a whitespace-separated list.
            if expected.is_empty() || expected.split_ascii_whitespace().count() != 1 {
                return false;
            }
            actual
                .split_ascii_whitespace()
                .any(|item| equal(item, expected, case_insensitive))
        }
        AttributeOperator::DashMatch => {
            equal(actual, expected, case_insensitive) || {
                starts_with(actual, expected, case_insensitive)
                    && actual.as_bytes().get(expected.len()) == Some(&b'-')
            }
        }
        AttributeOperator::Prefix => {
            !expected.is_empty() && starts_with(actual, expected, case_insensitive)
        }
        AttributeOperator::Suffix => {
            !expected.is_empty() && ends_with(actual, expected, case_insensitive)
        }
        AttributeOperator::Substring => {
            !expected.is_empty() && contains(actual, expected, case_insensitive)
        }
    }
}

/// Matches a pseudo-class.
fn matches_pseudo_class(document: &Document, node: NodeId, pseudo_class: &PseudoClass) -> bool {
    match pseudo_class {
        PseudoClass::Is(selectors) | PseudoClass::Where(selectors) => selectors
            .iter()
            .any(|selector| matches(document, node, selector)),
        PseudoClass::Not(selectors) => !selectors
            .iter()
            .any(|selector| matches(document, node, selector)),
        // Relative selectors need forward traversal, which this matcher does not do.
        PseudoClass::Has(_) => false,
        PseudoClass::Other { name, argument } => match (name.as_str(), argument.as_deref()) {
            ("root", None) => parent(document, node) == Some(document.root()),
            ("first-child", None) => previous_element_sibling(document, node).is_none(),
            ("last-child", None) => next_element_sibling(document, node).is_none(),
            ("nth-child", Some(argument)) => parse_an_plus_b(argument)
                .is_some_and(|(a, b)| matches_an_plus_b(element_index(document, node), a, b)),
            _ => false,
        },
    }
}

/// The parent of `node`, whatever kind it is.
fn parent(document: &Document, node: NodeId) -> Option<NodeId> {
    document.try_node(node)?.parent()
}

/// The parent of `node`, when it is an element.
fn parent_element(document: &Document, node: NodeId) -> Option<NodeId> {
    let parent = parent(document, node)?;
    document.element(parent).map(|_| parent)
}

/// The nearest previous sibling that is an element.
fn previous_element_sibling(document: &Document, node: NodeId) -> Option<NodeId> {
    let mut current = document.try_node(node)?.prev_sibling();
    while let Some(candidate) = current {
        if document.element(candidate).is_some() {
            return Some(candidate);
        }
        current = document.try_node(candidate)?.prev_sibling();
    }
    None
}

/// The nearest following sibling that is an element.
fn next_element_sibling(document: &Document, node: NodeId) -> Option<NodeId> {
    let mut current = document.try_node(node)?.next_sibling();
    while let Some(candidate) = current {
        if document.element(candidate).is_some() {
            return Some(candidate);
        }
        current = document.try_node(candidate)?.next_sibling();
    }
    None
}

/// The 1-based position of `node` among its element siblings.
fn element_index(document: &Document, node: NodeId) -> u32 {
    let mut index = 1_u32;
    let mut current = node;
    while let Some(previous) = previous_element_sibling(document, current) {
        index = index.saturating_add(1);
        current = previous;
    }
    index
}

/// Parses the `An+B` micro syntax of `:nth-child()`.
fn parse_an_plus_b(argument: &str) -> Option<(i32, i32)> {
    let mut input = cssparser::Parser::new(argument);
    input
        .parse_entirely(|input| parse_nth(input).map_err(cssparser::ParseError::<()>::from))
        .ok()
}

/// Returns `true` when a 1-based `index` satisfies `An+B` for some integer `n >= 0`.
fn matches_an_plus_b(index: u32, a: i32, b: i32) -> bool {
    if a == 0 {
        return i64::from(index) == i64::from(b);
    }
    let difference = i64::from(index) - i64::from(b);
    let step = i64::from(a);
    difference % step == 0 && difference / step >= 0
}

/// ASCII-case-aware equality.
fn equal(left: &str, right: &str, case_insensitive: bool) -> bool {
    if case_insensitive {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

/// ASCII-case-aware prefix test.
fn starts_with(haystack: &str, needle: &str, case_insensitive: bool) -> bool {
    if !case_insensitive {
        return haystack.starts_with(needle);
    }
    // Comparing byte slices keeps this correct even when the needle ends in the
    // middle of a multi-byte character that cannot match ASCII anyway.
    haystack.len() >= needle.len()
        && haystack.as_bytes()[..needle.len()].eq_ignore_ascii_case(needle.as_bytes())
}

/// ASCII-case-aware suffix test.
fn ends_with(haystack: &str, needle: &str, case_insensitive: bool) -> bool {
    if !case_insensitive {
        return haystack.ends_with(needle);
    }
    haystack.len() >= needle.len()
        && haystack.as_bytes()[haystack.len() - needle.len()..]
            .eq_ignore_ascii_case(needle.as_bytes())
}

/// ASCII-case-aware substring test.
fn contains(haystack: &str, needle: &str, case_insensitive: bool) -> bool {
    if !case_insensitive {
        return haystack.contains(needle);
    }
    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_plus_b_matches_first_child_syntaxes() {
        // `:nth-child(2n+1)` is the odd elements.
        assert!(matches_an_plus_b(1, 2, 1));
        assert!(!matches_an_plus_b(2, 2, 1));
        assert!(matches_an_plus_b(3, 2, 1));
        // `:nth-child(3)` is exactly the third.
        assert!(!matches_an_plus_b(2, 0, 3));
        assert!(matches_an_plus_b(3, 0, 3));
        // `:nth-child(-n+2)` is the first two.
        assert!(matches_an_plus_b(1, -1, 2));
        assert!(matches_an_plus_b(2, -1, 2));
        assert!(!matches_an_plus_b(3, -1, 2));
        // `:nth-child(0n+0)` matches nothing.
        assert!(!matches_an_plus_b(1, 0, 0));
    }

    #[test]
    fn an_plus_b_is_parsed_from_text() {
        assert_eq!(parse_an_plus_b("odd"), Some((2, 1)));
        assert_eq!(parse_an_plus_b("even"), Some((2, 0)));
        assert_eq!(parse_an_plus_b("2n+1"), Some((2, 1)));
        assert_eq!(parse_an_plus_b("3"), Some((0, 3)));
        assert_eq!(parse_an_plus_b("-n+2"), Some((-1, 2)));
        assert_eq!(parse_an_plus_b("garbage"), None);
    }

    #[test]
    fn case_insensitive_helpers_behave_like_their_plain_versions() {
        assert!(equal("AbC", "abc", true));
        assert!(!equal("AbC", "abc", false));
        assert!(starts_with("Hello", "he", true));
        assert!(!starts_with("Hello", "he", false));
        assert!(ends_with("Hello", "LO", true));
        assert!(!ends_with("Hello", "LO", false));
        assert!(contains("Hello", "ELL", true));
        assert!(!contains("Hello", "ELL", false));
        assert!(!contains("ab", "abc", true));
    }
}
