//! Selector matching against a real document.

mod common;

use common::{assert_selects, by_id, by_tag, element, elements, matches_selector, parse_html};
use jkva_dom::parse_document;
use jkva_style::{matches, matches_any, parse_selector_list};

/// A fixture with one element of every kind the matcher cares about.
///
/// ```html
/// <div id=app class="shell main" data-kind="alpha-1">
///   <p id=first class=note>a</p>
///   <p id=second data-kind="alphabet">b</p>
///   <span id=third class="note highlighted">c</span>
/// </div>
/// ```
fn fixture() -> jkva_dom::Document {
    parse_html(
        r#"<div id="app" class="shell main" data-kind="alpha-1">
             <p id="first" class="note">a</p>
             <p id="second" data-kind="alphabet">b</p>
             <span id="third" class="note highlighted">c</span>
           </div>"#,
    )
}

#[test]
fn universal_type_id_and_class_selectors() {
    let document = fixture();

    assert_selects(&document, "*", &["app", "first", "second", "third"]);
    assert_selects(&document, "p", &["first", "second"]);
    assert_selects(&document, "span", &["third"]);
    assert_selects(&document, "#app", &["app"]);
    assert_selects(&document, ".note", &["first", "third"]);
    assert_selects(&document, ".shell.main", &["app"]);
    assert_selects(&document, "p.note", &["first"]);
    assert_selects(&document, ".missing", &[]);
    // Type selectors are ASCII case-insensitive for HTML elements.
    assert_selects(&document, "P", &["first", "second"]);
}

#[test]
fn class_matching_uses_the_whole_word() {
    let document = fixture();
    assert_selects(&document, ".not", &[]);
    assert_selects(&document, ".high", &[]);
    assert_selects(&document, ".highlighted", &["third"]);
}

#[test]
fn attribute_selectors_support_all_six_operators() {
    let document = fixture();

    // Presence.
    assert_selects(&document, "[data-kind]", &["app", "second"]);
    // Equality.
    assert_selects(&document, "[data-kind=alphabet]", &["second"]);
    assert_selects(&document, "[data-kind=\"alpha-1\"]", &["app"]);
    // Whitespace-separated word.
    assert_selects(&document, "[class~=main]", &["app"]);
    assert_selects(&document, "[class~=mai]", &[]);
    // Dash match: the value is a prefix followed by `-`.
    assert_selects(&document, "[data-kind|=alpha]", &["app"]);
    // Prefix, suffix, substring.
    assert_selects(&document, "[data-kind^=alpha]", &["app", "second"]);
    assert_selects(&document, "[data-kind$=bet]", &["second"]);
    // A lone `-` is a delim token, not an identifier, so it has to be quoted.
    assert_selects(&document, r#"[data-kind*="-"]"#, &["app"]);
    // Empty values never match these three.
    assert_selects(&document, "[data-kind^=\"\"]", &[]);
    assert_selects(&document, "[data-kind$=\"\"]", &[]);
    assert_selects(&document, "[data-kind*=\"\"]", &[]);
}

#[test]
fn attribute_selectors_honour_the_case_insensitivity_flag() {
    let document = parse_document(r#"<div id="a" data-kind="Alpha-Beta"></div>"#);

    assert_selects(&document, "[data-kind=alpha-beta]", &[]);
    assert_selects(&document, "[data-kind=alpha-beta i]", &["a"]);
    assert_selects(&document, "[data-kind^=ALPHA i]", &["a"]);
    assert_selects(&document, "[data-kind^=ALPHA]", &[]);
    assert_selects(&document, "[data-kind$=BETA i]", &["a"]);
    assert_selects(&document, "[data-kind*=PHA i]", &["a"]);

    // HTML attribute names are case-insensitive even when the selector is not.
    assert_selects(&document, "[DATA-KIND]", &["a"]);
}

#[test]
fn is_where_and_not() {
    let document = fixture();

    assert_selects(&document, ":is(p, span)", &["first", "second", "third"]);
    assert_selects(&document, ":where(.note, #app)", &["app", "first", "third"]);
    assert_selects(&document, ":not(p)", &["app", "third"]);
    assert_selects(&document, "p:not(.note)", &["second"]);
    assert_selects(
        &document,
        "#app :is(p, span)",
        &["first", "second", "third"],
    );
}

#[test]
fn structural_pseudo_classes() {
    let document = parse_document(
        r#"<ul><li id="a">a</li><li id="b">b</li><li id="c">c</li><li id="d">d</li></ul>"#,
    );

    assert_selects(&document, "li:first-child", &["a"]);
    assert_selects(&document, "li:last-child", &["d"]);
    assert_selects(&document, "li:nth-child(2)", &["b"]);
    assert_selects(&document, "li:nth-child(odd)", &["a", "c"]);
    assert_selects(&document, "li:nth-child(even)", &["b", "d"]);
    assert_selects(&document, "li:nth-child(2n+1)", &["a", "c"]);
    assert_selects(&document, "li:nth-child(n+3)", &["c", "d"]);
    assert_selects(&document, "li:nth-child(-n+2)", &["a", "b"]);
    assert_selects(&document, "li:nth-child(0n+0)", &[]);

    // The root element is the only element child of the document node.
    let list = by_tag(&document, "ul");
    assert!(
        !matches_selector(&document, list, ":root"),
        "the ul is not the root element"
    );
    let root = by_tag(&document, "html");
    assert!(matches_selector(&document, root, ":root"));
    assert!(matches_selector(&document, root, "html:first-child"));
}

#[test]
fn text_and_comment_nodes_do_not_match() {
    let document = parse_document("<p id=a>hello<!-- note --></p>");
    let paragraph = by_id(&document, "a");

    // The text and comment children are not elements and match no selector.
    let children: Vec<_> = document.children(paragraph).collect();
    assert!(!children.is_empty());
    let selector = parse_selector_list("*").unwrap();
    for child in children {
        assert!(document.element(child).is_none());
        assert!(!matches(&document, child, &selector[0]));
    }
}

#[test]
fn combinators() {
    let document = fixture();

    // Descendant.
    assert_selects(&document, "#app p", &["first", "second"]);
    assert_selects(&document, "#app .note", &["first", "third"]);
    // Child.
    assert_selects(&document, "#app > p", &["first", "second"]);
    assert_selects(&document, "#app > span", &["third"]);
    // No grandchildren in this fixture, so these are about the axis, not depth.
    assert_selects(&document, "div > div", &[]);
}

#[test]
fn descendant_versus_child_is_distinguished() {
    // `<div><section><p></p></section></div>`:
    let document =
        parse_document(r#"<div id="outer"><section id="mid"><p id="inner">x</p></section></div>"#);

    assert_selects(&document, "#outer p", &["inner"]);
    assert_selects(&document, "#outer > p", &[]);
    assert_selects(&document, "div p", &["inner"]);
    assert_selects(&document, "div > section > p", &["inner"]);
    assert_selects(&document, "div > p", &[]);
}

#[test]
fn sibling_combinators() {
    let document =
        parse_document(r#"<ul><li id="a">a</li><li id="b">b</li><li id="c">c</li></ul>"#);

    // Next-sibling: only the immediate follower.
    assert_selects(&document, "#a + li", &["b"]);
    assert_selects(&document, "li + li", &["b", "c"]);
    assert_selects(&document, "li + #c", &["c"]);
    // Later-sibling: any follower.
    assert_selects(&document, "#a ~ li", &["b", "c"]);
    assert_selects(&document, "#b ~ li", &["c"]);
    assert_selects(&document, "#c ~ li", &[]);
    // Mixed with a descendant step.
    assert_selects(&document, "ul > #a + li ~ li", &["c"]);
}

#[test]
fn unsupported_pseudo_classes_never_match() {
    let document = fixture();
    let paragraph = by_id(&document, "first");

    for selector in [
        "p:hover",
        "p:focus",
        "p:first-of-type",
        "p:nth-last-child(1)",
        "p:has(span)",
        "p::before",
    ] {
        assert!(
            !matches_selector(&document, paragraph, selector),
            "{selector} should not match"
        );
    }

    // `:hover` cannot be evaluated, so `:not(:hover)` inverts to "matches".
    // This is documented as unsound in general and only intuitive for state.
    assert!(matches_selector(&document, paragraph, "p:not(:hover)"));
}

#[test]
fn namespace_prefixes_never_match() {
    let document = fixture();
    for selector in ["svg|p", "*|p", "|p"] {
        assert_selects(&document, selector, &[]);
    }
}

#[test]
fn a_stale_node_does_not_panic() {
    let mut document = parse_document("<p id=a>hello</p>");
    let paragraph = by_id(&document, "a");
    let selector = parse_selector_list("p").unwrap();

    assert!(matches(&document, paragraph, &selector[0]));
    document.remove_node(paragraph);
    // The handle is stale now; matching must report false rather than panic.
    assert!(!matches(&document, paragraph, &selector[0]));
}

#[test]
fn matches_any_accepts_a_selector_list() {
    let document = fixture();
    let second = by_id(&document, "second");

    let selectors = parse_selector_list("#nope, .nope, p").unwrap();
    assert!(matches_any(&document, second, &selectors));
}

#[test]
fn every_element_of_the_fixture_is_reachable_by_selector() {
    let document = fixture();
    let all = elements(&document);
    // html, head, body plus the four authored elements.
    assert_eq!(all.len(), 7);
    for node in all {
        let name = element(&document, node).name().to_owned();
        assert!(matches_selector(&document, node, &name), "<{name}>");
    }
}
