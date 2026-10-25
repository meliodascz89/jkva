//! The rule index: bucketing must narrow the work without changing any result.
//!
//! Every test here is written to fail if the index *under*-approximates — that is,
//! if a selector that should have been considered for an element is never looked
//! at. Over-approximation is harmless, because [`jkva_style::matches`] stays
//! authoritative.

mod common;

use common::{by_id, engine_of, parse_html, style_of};
use jkva_style::{StyleEngine, Stylesheet};

#[test]
fn a_descendant_combinator_rule_still_wins() {
    // The rightmost compound is `p`, so the selector is filed under type `p`; the
    // ancestor part still has to be evaluated by the matcher.
    let document = parse_html(
        r#"<div id="in"><p id="inside">x</p></div>
           <section><p id="outside">y</p></section>"#,
    );
    let engine = engine_of("#in p { color: red }");
    let styles = engine.compute_styles(&document);

    assert_eq!(style_of(&document, &styles, "inside").color, "red");
    assert_eq!(style_of(&document, &styles, "outside").color, "black");
}

#[test]
fn child_and_sibling_combinators_still_win() {
    let document = parse_html(
        r#"<div id="parent"><span id="child">x</span></div>
           <ul><li id="first">a</li><li id="second">b</li><li id="third">c</li></ul>"#,
    );
    let engine =
        engine_of("#parent > span { color: red } li:first-child + li { background-color: yellow }");
    let styles = engine.compute_styles(&document);

    assert_eq!(style_of(&document, &styles, "child").color, "red");
    assert_eq!(
        style_of(&document, &styles, "first").background_color,
        "transparent"
    );
    assert_eq!(
        style_of(&document, &styles, "second").background_color,
        "yellow"
    );
    assert_eq!(
        style_of(&document, &styles, "third").background_color,
        "transparent"
    );
}

#[test]
fn a_pseudo_class_rule_still_wins() {
    // `li:first-child` is filed under type `li` even though the compound also
    // carries a pseudo-class.
    let document = parse_html(r#"<ul><li id="a">a</li><li id="b">b</li></ul>"#);
    let engine = engine_of("li:first-child { color: red }");
    let styles = engine.compute_styles(&document);

    assert_eq!(style_of(&document, &styles, "a").color, "red");
    assert_eq!(style_of(&document, &styles, "b").color, "black");
}

#[test]
fn a_pseudo_class_without_any_indexable_part_uses_the_universal_bucket() {
    // `:first-child` alone has no id, class or type, so it must be tested for
    // every element.
    let document = parse_html(r#"<div><p id="first">x</p><p id="second">y</p></div>"#);
    // A non-inherited property, so the assertion cannot be satisfied by
    // inheritance from the first-child <div>.
    let engine = engine_of(":first-child { background-color: yellow }");
    let styles = engine.compute_styles(&document);

    // Both the div and the first paragraph are first children; the second
    // paragraph is not.
    assert_eq!(
        style_of(&document, &styles, "first").background_color,
        "yellow"
    );
    assert_eq!(
        style_of(&document, &styles, "second").background_color,
        "transparent"
    );
}

#[test]
fn an_attribute_selector_rule_still_wins() {
    // `div[data-on]` is filed under type `div`; the attribute is re-tested.
    let document = parse_html(
        r#"<div id="on" data-on></div>
           <div id="off"></div>"#,
    );
    let engine = engine_of("div[data-on] { color: red }");
    let styles = engine.compute_styles(&document);

    assert_eq!(style_of(&document, &styles, "on").color, "red");
    assert_eq!(style_of(&document, &styles, "off").color, "black");
}

#[test]
fn an_attribute_only_compound_uses_the_universal_bucket() {
    let document = parse_html(
        r#"<span id="on" data-on></span>
           <span id="off"></span>"#,
    );
    let engine = engine_of("[data-on] { color: red }");
    let styles = engine.compute_styles(&document);

    assert_eq!(style_of(&document, &styles, "on").color, "red");
    assert_eq!(style_of(&document, &styles, "off").color, "black");
}

#[test]
fn a_universal_selector_and_an_id_ancestor_still_work() {
    // The rightmost compound is `*`, which is not indexable.
    let document = parse_html(
        r#"<div id="in"><span id="inside">x</span></div>
           <span id="outside">y</span>"#,
    );
    let engine = engine_of("#in * { color: red } * { background-color: yellow }");
    let styles = engine.compute_styles(&document);

    assert_eq!(style_of(&document, &styles, "inside").color, "red");
    assert_eq!(style_of(&document, &styles, "outside").color, "black");
    // The bare universal rule reaches every element.
    for id in ["inside", "outside"] {
        assert_eq!(
            style_of(&document, &styles, id).background_color,
            "yellow",
            "#{id}"
        );
    }
}

#[test]
fn an_element_in_two_class_buckets_is_found_from_either() {
    // The required case: both class buckets must contribute.
    let document = parse_html(r#"<div id="both" class="a b"></div>"#);
    let engine = engine_of(".a { color: red } .b { background-color: yellow }");
    let styles = engine.compute_styles(&document);

    let style = style_of(&document, &styles, "both");
    assert_eq!(style.color, "red", "the `a` bucket must be consulted");
    assert_eq!(
        style.background_color, "yellow",
        "the `b` bucket must be consulted"
    );
}

#[test]
fn a_class_repeated_in_the_attribute_does_not_change_the_result() {
    let document = parse_html(r#"<div id="d" class="a a b"></div>"#);
    let engine = engine_of(".a { color: red } .b { color: green }");
    let styles = engine.compute_styles(&document);

    assert_eq!(style_of(&document, &styles, "d").color, "green");
}

#[test]
fn specificity_decides_across_buckets_not_bucket_order() {
    let document = parse_html(r#"<div id="target" class="note"></div>"#);

    // Type bucket first, class bucket second: the class rule is more specific.
    let engine = engine_of("div { color: red } .note { color: green }");
    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "target").color,
        "green"
    );

    // Reversed declaration order: specificity still wins over source order.
    let engine = engine_of(".note { color: green } div { color: red }");
    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "target").color,
        "green"
    );

    // The id bucket is the most specific of all.
    let engine = engine_of("#target { color: blue } .note { color: green }");
    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "target").color,
        "blue"
    );
}

#[test]
fn source_order_decides_between_buckets_at_equal_specificity() {
    // Both selectors are class selectors of equal specificity, sitting in
    // different buckets: source order must win, whatever order the buckets are
    // visited in.
    let document = parse_html(r#"<div id="target" class="a b"></div>"#);

    let engine = engine_of(".a { color: red } .b { color: green }");
    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "target").color,
        "green"
    );

    let engine = engine_of(".b { color: green } .a { color: red }");
    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "target").color,
        "red"
    );
}

#[test]
fn important_wins_across_buckets() {
    let document = parse_html(r#"<div id="target" class="note"></div>"#);
    let engine = engine_of("div { color: red !important } #target.note { color: blue }");

    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "target").color,
        "red"
    );
}

#[test]
fn type_selectors_stay_case_insensitive_through_the_index() {
    // Type buckets are keyed by the lowercased name, so an uppercase selector
    // still finds a lowercase HTML element.
    let document = parse_html(r#"<div id="d"></div><x-widget id="w"></x-widget>"#);
    let engine = engine_of("DIV { color: red } X-WIDGET { color: blue }");
    let styles = engine.compute_styles(&document);

    assert_eq!(style_of(&document, &styles, "d").color, "red");
    assert_eq!(style_of(&document, &styles, "w").color, "blue");
}

#[test]
fn a_namespace_prefixed_type_selector_never_matches_but_stays_safe() {
    // A namespaced type is not indexable, so it lands in the universal bucket and
    // is rejected by the matcher; it must not disturb the rules around it.
    let document = parse_html(r#"<div id="target"></div>"#);
    let engine = engine_of("svg|rect { color: red } div { color: blue }");

    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "target").color,
        "blue"
    );
}

#[test]
fn the_index_is_extended_by_a_stylesheet_added_after_a_pass() {
    let document = parse_html(r#"<div id="target" class="later"></div>"#);
    let mut engine = engine_of(".later { color: red }");

    let before = engine.compute_styles(&document);
    assert_eq!(style_of(&document, &before, "target").color, "red");

    // A sheet added after the first pass must be indexed too.
    engine.add_stylesheet(Stylesheet::parse(".later { color: blue }").unwrap());
    let after = engine.compute_styles(&document);
    assert_eq!(
        style_of(&document, &after, "target").color,
        "blue",
        "the new sheet must take part in the cascade"
    );

    // ...and a brand-new selector from that sheet is reachable as well.
    engine.add_stylesheet(Stylesheet::parse("#brand-new { width: 42px }").unwrap());
    let document = parse_html(r#"<div id="brand-new"></div>"#);
    let after = engine.compute_styles(&document);
    assert_eq!(style_of(&document, &after, "brand-new").width, "42px");
}

#[test]
fn an_engine_without_sheets_has_an_empty_index() {
    let document = parse_html(r#"<div id="d"></div>"#);
    let engine = StyleEngine::without_user_agent();
    let styles = engine.compute_styles(&document);

    // The style is still produced (initial values); the empty index simply
    // yields no candidates.
    assert!(styles.get(by_id(&document, "d")).is_some());
}
