//! End-to-end cascade tests: HTML plus CSS in, computed styles out.

mod common;

use common::{by_id, elements, engine_from, engine_of, parse_html, style_of};
use jkva_dom::parse_document;
use jkva_style::{ComputedStyle, DisplayType, StyleEngine, Stylesheet};

#[test]
fn every_tracked_property_computes_from_a_declaration() {
    let document = parse_html(r#"<div id="box">x</div>"#);
    let engine = engine_of(
        r#"
        div {
            display: block;
            color: red;
            background-color: #eeeeee;
            width: 50%;
            height: 10px;
            font-size: 12pt;
            font-weight: 700;
            font-family: "Times New Roman";
            margin-top: 1px;
            margin-right: 2px;
            margin-bottom: 3px;
            margin-left: 4px;
            padding-top: 5px;
            padding-right: 6px;
            padding-bottom: 7px;
            padding-left: 8px;
        }
        "#,
    );
    let styles = engine.compute_styles(&document);
    let style = style_of(&document, &styles, "box");

    assert_eq!(style.display, DisplayType::Block);
    assert_eq!(style.color, "red");
    assert_eq!(style.background_color, "#eeeeee");
    assert_eq!(style.width, "50%");
    assert_eq!(style.height, "10px");
    assert_eq!(style.font_size, "12pt");
    assert_eq!(style.font_weight, "700");
    assert_eq!(style.font_family, r#""Times New Roman""#);
    assert_eq!(
        (
            style.margin_top.as_str(),
            style.margin_right.as_str(),
            style.margin_bottom.as_str(),
            style.margin_left.as_str(),
        ),
        ("1px", "2px", "3px", "4px")
    );
    assert_eq!(
        (
            style.padding_top.as_str(),
            style.padding_right.as_str(),
            style.padding_bottom.as_str(),
            style.padding_left.as_str(),
        ),
        ("5px", "6px", "7px", "8px")
    );
}

#[test]
fn unmatched_elements_get_the_initial_values() {
    let document = parse_html(r#"<div id="box">x</div>"#);
    let engine = engine_of("p { color: red }");
    let styles = engine.compute_styles(&document);

    assert_eq!(
        style_of(&document, &styles, "box"),
        ComputedStyle::initial()
    );
}

#[test]
fn specificity_beats_source_order() {
    let document = parse_html(r#"<p id="target" class="note">x</p>"#);
    let engine = engine_of(
        r#"
        p { color: red }
        .note { color: green }
        #target { color: blue }
        "#,
    );
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.color, "blue", "the id selector is the most specific");

    // Same declarations in the reverse order must give the same winner.
    let engine = engine_of(
        r#"
        #target { color: blue }
        .note { color: green }
        p { color: red }
        "#,
    );
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.color, "blue");
}

#[test]
fn source_order_breaks_ties_at_equal_specificity() {
    let document = parse_html(r#"<p id="target">x</p>"#);
    let engine = engine_of("p { color: red } p { color: green }");
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.color, "green", "the later declaration wins");

    // ... but only within the same selector: a later, weaker one still loses.
    let engine = engine_of("p.note { color: red } p { color: green }");
    let document = parse_html(r#"<p id="target" class="note">x</p>"#);
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.color, "red");
}

#[test]
fn important_beats_specificity() {
    let document = parse_html(r#"<p id="target" class="note">x</p>"#);
    let engine = engine_of(
        r#"
        p { color: red !important }
        #target.note { color: blue }
        "#,
    );
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.color, "red");

    // Two important declarations fall back to specificity.
    let engine = engine_of(
        r#"
        p { color: red !important }
        #target { color: blue !important }
        "#,
    );
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.color, "blue");
}

#[test]
fn inherited_properties_come_from_the_parent() {
    let document = parse_html(
        r#"<div id="parent">
             <span id="child">x</span>
             <span id="other" class="own">y</span>
           </div>"#,
    );
    let engine = engine_of(
        r#"
        #parent {
            color: navy;
            font-size: 20px;
            font-weight: 600;
            font-family: Georgia;
            margin-top: 5px;
            background-color: yellow;
            display: block;
        }
        .own { color: teal }
        "#,
    );
    let styles = engine.compute_styles(&document);
    let child = style_of(&document, &styles, "child");

    // Inherited.
    assert_eq!(child.color, "navy");
    assert_eq!(child.font_size, "20px");
    assert_eq!(child.font_weight, "600");
    assert_eq!(child.font_family, "Georgia");
    // Not inherited: these fall back to their initial values.
    assert_eq!(child.margin_top, "0");
    assert_eq!(child.background_color, "transparent");
    assert_eq!(child.display, DisplayType::Inline);

    // A child rule still overrides the inherited value.
    assert_eq!(style_of(&document, &styles, "other").color, "teal");
}

#[test]
fn inheritance_starts_from_the_initial_values_at_the_root() {
    let document = parse_html("<p id=leaf>x</p>");
    let engine = engine_of("span { color: red }");
    let style = style_of(&document, &engine.compute_styles(&document), "leaf");
    assert_eq!(style.color, "black");
}

#[test]
fn descendant_and_child_combinators_differ() {
    let document = parse_html(
        r#"<div id="outer">
             <section><p id="deep">deep</p></section>
             <p id="direct">direct</p>
           </div>"#,
    );
    let engine = engine_of("#outer p { color: red } #outer > p { color: green }");

    let styles = engine.compute_styles(&document);
    // Equal specificity, so the later `>` rule wins where both apply.
    assert_eq!(style_of(&document, &styles, "deep").color, "red");
    assert_eq!(style_of(&document, &styles, "direct").color, "green");
}

#[test]
fn sibling_combinators_drive_the_cascade() {
    let document =
        parse_document(r#"<ul><li id="a">a</li><li id="b">b</li><li id="c">c</li></ul>"#);
    let engine = engine_of("li + li { color: red } #b ~ li { background-color: yellow }");
    let styles = engine.compute_styles(&document);

    assert_eq!(style_of(&document, &styles, "a").color, "black");
    assert_eq!(style_of(&document, &styles, "b").color, "red");
    assert_eq!(style_of(&document, &styles, "c").color, "red");
    assert_eq!(
        style_of(&document, &styles, "b").background_color,
        "transparent"
    );
    assert_eq!(style_of(&document, &styles, "c").background_color, "yellow");
}

#[test]
fn attribute_selectors_drive_the_cascade() {
    let document = parse_html(
        r#"<div id="alpha" data-kind="alpha"></div>
           <div id="beta" data-kind="beta"></div>"#,
    );
    let engine = engine_of("[data-kind^=alph] { color: red } [data-kind=beta i] { color: blue }");
    let styles = engine.compute_styles(&document);

    assert_eq!(style_of(&document, &styles, "alpha").color, "red");
    assert_eq!(style_of(&document, &styles, "beta").color, "blue");
}

#[test]
fn a_rule_inside_media_is_ignored() {
    // Documented behaviour: at-rules are recorded and skipped, including their
    // contents, so the `@media` rule never reaches the cascade.
    let css = r#"
        @media (min-width: 600px) { p { color: red } }
        p { color: blue }
    "#;
    let sheet = Stylesheet::parse(css).expect("parses");
    assert_eq!(sheet.at_rules().len(), 1, "the at-rule is recorded");
    assert_eq!(
        sheet.rules().len(),
        1,
        "only the unconditional rule survives"
    );

    let document = parse_html("<p id=target>x</p>");
    let engine = StyleEngine::from_stylesheets([sheet]);
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.color, "blue");
}

#[test]
fn margin_and_padding_shorthands_expand_to_four_sides() {
    let document = parse_html(
        r#"<div id="one"></div><div id="two"></div>
           <div id="three"></div><div id="four"></div><div id="bad"></div>"#,
    );
    let engine = engine_of(
        r#"
        #one { margin: 1px }
        #two { margin: 1px 2px }
        #three { margin: 1px 2px 3px }
        #four { margin: 1px 2px 3px 4px }
        #bad { margin: 1px 2px 3px 4px 5px }
        div { padding: 1px 2px }
        "#,
    );
    let styles = engine.compute_styles(&document);

    let one = style_of(&document, &styles, "one");
    assert_eq!(
        [
            one.margin_top.as_str(),
            one.margin_right.as_str(),
            one.margin_bottom.as_str(),
            one.margin_left.as_str(),
        ],
        ["1px", "1px", "1px", "1px"]
    );

    let two = style_of(&document, &styles, "two");
    assert_eq!(
        [
            two.margin_top.as_str(),
            two.margin_right.as_str(),
            two.margin_bottom.as_str(),
            two.margin_left.as_str(),
        ],
        ["1px", "2px", "1px", "2px"]
    );

    let three = style_of(&document, &styles, "three");
    assert_eq!(
        [
            three.margin_top.as_str(),
            three.margin_right.as_str(),
            three.margin_bottom.as_str(),
            three.margin_left.as_str(),
        ],
        ["1px", "2px", "3px", "2px"]
    );

    let four = style_of(&document, &styles, "four");
    assert_eq!(
        [
            four.margin_top.as_str(),
            four.margin_right.as_str(),
            four.margin_bottom.as_str(),
            four.margin_left.as_str(),
        ],
        ["1px", "2px", "3px", "4px"]
    );

    // Five components are invalid, so that declaration is dropped entirely.
    let bad = style_of(&document, &styles, "bad");
    assert_eq!(bad.margin_top, "0");

    // The same expansion applies to `padding`, and shorthands keep their
    // function arguments intact.
    let one = style_of(&document, &styles, "one");
    assert_eq!(
        [one.padding_top.as_str(), one.padding_left.as_str()],
        ["1px", "2px"]
    );
}

#[test]
fn longhands_and_shorthands_compete_by_source_order() {
    let document = parse_html(r#"<div id="target"></div>"#);

    // Longhand after shorthand wins.
    let engine = engine_of("#target { margin: 5px; margin-top: 1px }");
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.margin_top, "1px");
    assert_eq!(style.margin_left, "5px");

    // Shorthand after longhand wins, because it sets every side.
    let engine = engine_of("#target { margin-top: 1px; margin: 5px }");
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.margin_top, "5px");
}

#[test]
fn css_wide_keywords_are_resolved() {
    let document = parse_html(
        r#"<div id="parent">
             <span id="inherit"></span>
             <span id="initial"></span>
             <span id="unset"></span>
           </div>"#,
    );
    let engine = engine_of(
        r#"
        #parent { color: navy; margin-top: 3px }
        #inherit { color: inherit; margin-top: inherit }
        #initial { color: initial; margin-top: initial }
        #unset { color: unset; margin-top: unset }
        "#,
    );
    let styles = engine.compute_styles(&document);

    let inherit = style_of(&document, &styles, "inherit");
    assert_eq!(inherit.color, "navy");
    // `inherit` works for non-inherited properties too.
    assert_eq!(inherit.margin_top, "3px");

    let initial = style_of(&document, &styles, "initial");
    assert_eq!(initial.color, "black");
    assert_eq!(initial.margin_top, "0");

    let unset = style_of(&document, &styles, "unset");
    // `color` inherits, so `unset` means "take the parent's value".
    assert_eq!(unset.color, "navy");
    // `margin-top` does not inherit, so `unset` means "initial".
    assert_eq!(unset.margin_top, "0");
}

#[test]
fn style_sheets_are_applied_in_cascade_order() {
    let document = parse_html("<p id=target>x</p>");
    let engine = engine_from(&["p { color: red }", "p { color: green }"]);
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.color, "green");

    // A later sheet with a weaker selector still loses on specificity.
    let engine = engine_from(&["#target { color: red }", "p { color: green }"]);
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.color, "red");
}

#[test]
fn var_is_left_unresolved() {
    let document = parse_html("<p id=target>x</p>");
    let engine = engine_of("p { color: var(--brand); width: var(--w, 10px) }");
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.color, "var(--brand)");
    assert_eq!(style.width, "var(--w, 10px)");
}

#[test]
fn unknown_properties_and_unexpanded_shorthands_are_ignored() {
    let document = parse_html("<p id=target>x</p>");
    let engine = engine_of("p { position: absolute; background: red; color: blue }");
    let style = style_of(&document, &engine.compute_styles(&document), "target");

    assert_eq!(style.color, "blue");
    // `background` is not expanded, so `background-color` keeps its initial value.
    assert_eq!(style.background_color, "transparent");
}

#[test]
fn an_invalid_display_value_is_dropped() {
    let document = parse_html("<p id=target>x</p>");

    // Alone, an invalid keyword leaves the initial value in place.
    let engine = engine_of("p { display: bogus }");
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.display, DisplayType::Inline);

    // A valid earlier declaration still wins over a later invalid one.
    let engine = engine_of("p { display: inline-block } p { display: bogus }");
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.display, DisplayType::InlineBlock);
}

#[test]
fn display_can_be_inherited_explicitly() {
    let document = parse_html(r#"<div id="parent"><span id="child"></span></div>"#);
    let engine = engine_of("#parent { display: flex } #child { display: inherit }");
    let styles = engine.compute_styles(&document);

    assert_eq!(
        style_of(&document, &styles, "parent").display,
        DisplayType::Flex
    );
    assert_eq!(
        style_of(&document, &styles, "child").display,
        DisplayType::Flex,
        "display inherits only when asked to"
    );
}

#[test]
fn where_contributes_no_specificity_and_is_contributes_its_best_argument() {
    let document = parse_html(r#"<p id="target" class="note">x</p>"#);

    // `:where()` is zero-specificity, so the plain `p` rule wins despite coming
    // first.
    let engine = engine_of("p { color: blue } :where(#target) { color: red }");
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.color, "blue");

    // `:is()` takes the specificity of its most specific argument, which is the
    // id here, so it beats a plain type selector.
    let engine = engine_of("p { color: blue } :is(#target, .note) { color: red }");
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.color, "red");
}

#[test]
fn the_map_covers_exactly_the_element_nodes() {
    let document = parse_html("<div id=box>text<!-- comment --></div>");
    let engine = engine_of("div { color: red }");
    let styles = engine.compute_styles(&document);

    assert_eq!(styles.len(), elements(&document).len());
    assert!(!styles.is_empty());

    // Text and comment nodes are not styled.
    let container = by_id(&document, "box");
    for child in document.children(container) {
        assert!(document.element(child).is_none());
        assert!(styles.get(child).is_none());
    }

    // The convenience accessor agrees with the map.
    let single = engine
        .computed_style(&document, container)
        .expect("the box is styled");
    assert_eq!(Some(&single), styles.get(container));
}

#[test]
fn an_engine_without_stylesheets_computes_initial_values() {
    let document = parse_html("<p id=target>x</p>");
    let engine = StyleEngine::new();
    assert!(engine.sheets().is_empty());

    let styles = engine.compute_styles(&document);
    assert_eq!(
        style_of(&document, &styles, "target"),
        ComputedStyle::initial()
    );
}
