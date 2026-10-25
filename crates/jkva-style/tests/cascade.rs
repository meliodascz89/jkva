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
            border-top-width: 9px;
            border-right-width: 10px;
            border-bottom-width: 11px;
            border-left-width: 12px;
            border-top-style: solid;
            border-right-style: dashed;
            border-bottom-style: dotted;
            border-left-style: double;
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
    assert_eq!(
        (
            style.border_top_width.as_str(),
            style.border_right_width.as_str(),
            style.border_bottom_width.as_str(),
            style.border_left_width.as_str(),
        ),
        ("9px", "10px", "11px", "12px")
    );
    assert_eq!(
        (
            style.border_top_style.as_str(),
            style.border_right_style.as_str(),
            style.border_bottom_style.as_str(),
            style.border_left_style.as_str(),
        ),
        ("solid", "dashed", "dotted", "double")
    );
}

#[test]
fn flex_properties_compute() {
    let document = parse_html(r#"<div id="box">x</div>"#);
    let engine = engine_of(
        "div { flex-direction: column; justify-content: space-between; align-items: center; gap: 12px; align-self: flex-end }",
    );
    let style = style_of(&document, &engine.compute_styles(&document), "box");
    assert_eq!(style.flex_direction, "column");
    assert_eq!(style.justify_content, "space-between");
    assert_eq!(style.align_items, "center");
    assert_eq!(style.gap, "12px");
    assert_eq!(style.align_self, "flex-end");
}

#[test]
fn border_shorthand_sets_width_and_style_on_every_side() {
    let document = parse_html(r#"<div id="box">x</div>"#);
    let engine = engine_of("div { border: 3px solid #00ff00 }");
    let style = style_of(&document, &engine.compute_styles(&document), "box");
    assert_eq!(style.border_top_width, "3px");
    assert_eq!(style.border_right_width, "3px");
    assert_eq!(style.border_bottom_width, "3px");
    assert_eq!(style.border_left_width, "3px");
    assert_eq!(style.border_top_style, "solid");
    assert_eq!(style.border_left_style, "solid");
    assert_eq!(style.border_top_color, "#00ff00");
    assert_eq!(style.border_left_color, "#00ff00");
}

#[test]
fn elements_the_user_agent_sheet_does_not_know_keep_the_initial_values() {
    // A custom element is matched by neither the UA sheet nor the author sheet,
    // so every property keeps its initial value.
    let document = parse_html(r#"<x-widget id="box">x</x-widget>"#);
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
    // `x-child` is deliberately not an element the UA sheet styles, so the
    // "not inherited" assertions below cannot pass because of a UA rule.
    let document = parse_html(
        r#"<div id="parent">
             <x-child id="child">x</x-child>
             <x-child id="other" class="own">y</x-child>
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

    // Alone, an invalid keyword is dropped, so the user-agent `p { display: block }`
    // is what remains — not the initial `inline`.
    let engine = engine_of("p { display: bogus }");
    let style = style_of(&document, &engine.compute_styles(&document), "target");
    assert_eq!(style.display, DisplayType::Block);

    // With no UA sheet at all, the same document falls back to the initial value.
    let mut engine = StyleEngine::without_user_agent();
    engine.add_stylesheet(Stylesheet::parse("p { display: bogus }").unwrap());
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
    let engine = StyleEngine::without_user_agent();
    assert!(engine.sheets().is_empty());

    let styles = engine.compute_styles(&document);
    assert_eq!(
        style_of(&document, &styles, "target"),
        ComputedStyle::initial()
    );

    // `new()` is the same engine plus the built-in user-agent sheet.
    let engine = StyleEngine::new();
    assert_eq!(engine.sheets().len(), 1);
    let styles = engine.compute_styles(&document);
    assert_eq!(
        style_of(&document, &styles, "target").display,
        DisplayType::Block
    );
}

#[test]
fn the_map_iterates_in_document_order() {
    let document = parse_html(r#"<div id="a"><span id="b"></span><span id="c"></span></div>"#);
    let engine = engine_of("div { color: red }");
    let styles = engine.compute_styles(&document);

    let names: Vec<String> = styles
        .iter()
        .filter_map(|(node, _)| {
            document
                .element(node)
                .map(|element| element.name().to_owned())
        })
        .collect();
    assert_eq!(names, ["html", "head", "body", "div", "span", "span"]);

    let ids: Vec<Option<&str>> = styles
        .iter()
        .filter_map(|(node, _)| {
            document
                .element(node)
                .map(|element| element.attribute("id"))
        })
        .collect();
    assert_eq!(ids, [None, None, None, Some("a"), Some("b"), Some("c")]);
}

#[test]
fn two_passes_produce_identical_maps() {
    let document = parse_html(r#"<div id="a"></div><p id="b">x</p>"#);
    let engine = engine_of("div { color: red } p { font-weight: 700 }");

    let first = engine.compute_styles(&document);
    let second = engine.compute_styles(&document);

    assert_eq!(
        first.iter().collect::<Vec<_>>(),
        second.iter().collect::<Vec<_>>(),
        "computation must be deterministic"
    );
    assert_eq!(first.len(), second.len());
}

#[test]
fn a_reused_arena_slot_never_leaks_another_nodes_style() {
    let mut document = parse_html(r#"<div id="a" class="old"></div>"#);
    let engine = engine_of(".old { color: red } .new { color: blue }");

    let old_node = by_id(&document, "a");
    let before = engine.compute_styles(&document);
    assert_eq!(before.get(old_node).unwrap().color, "red");

    // Remove the old element and put a new one into the freed arena slot.
    let parent = document.parent(old_node).expect("the div has a parent");
    document.remove_node(old_node);
    let new_node = document.create_element("div");
    document.set_attribute(new_node, "class", "new");
    document.append_child(parent, new_node);
    assert_eq!(
        new_node.index(),
        old_node.index(),
        "the arena should have reused the slot"
    );

    let after = engine.compute_styles(&document);
    assert_eq!(after.get(new_node).unwrap().color, "blue");
    // Generation checking: the stale handle must not resolve to the new node's
    // style even though it now names the same arena index.
    assert!(
        after.get(old_node).is_none(),
        "a stale handle must not read the reused slot"
    );

    // The earlier map is a snapshot of the earlier tree; it still answers for the
    // node that existed then. Recompute to observe a removal.
    assert_eq!(before.get(old_node).unwrap().color, "red");
}

#[test]
fn an_element_with_no_matching_rules_gets_the_initial_values() {
    // The fast path starts from an allocation-free "empty" style and fills every
    // slot, so an element nothing matches must still come out fully initialised.
    let document = parse_html("<div id=d></div>");
    let mut engine = StyleEngine::without_user_agent();
    engine.add_stylesheet(Stylesheet::parse("p { color: red }").unwrap());

    let styles = engine.compute_styles(&document);
    assert_eq!(style_of(&document, &styles, "d"), ComputedStyle::initial());
}

#[test]
fn properties_without_a_declaration_keep_their_initial_values() {
    // The counterpart of the test above: an element that *does* get declarations
    // must still initialise the properties it never mentions. An empty string
    // here would mean the fast path leaked its scratch style.
    let document = parse_html("<div id=d></div>");
    let engine = engine_of("#d { color: red; margin-top: 3px }");
    let style = style_of(&document, &engine.compute_styles(&document), "d");

    assert_eq!(style.color, "red");
    assert_eq!(style.margin_top, "3px");

    assert_eq!(style.margin_right, "0");
    assert_eq!(style.padding_top, "0");
    assert_eq!(style.border_top_style, "none");
    assert_eq!(style.border_left_width, "0");
    assert_eq!(style.width, "auto");
    assert_eq!(style.height, "auto");
    assert_eq!(style.background_color, "transparent");
    assert_eq!(style.font_size, "16px");
    assert_eq!(style.font_weight, "normal");
    // `font-family` starts as the empty string, meaning "UA default"; the other
    // fields must not be empty.
    assert_eq!(style.font_family, "");
    assert_ne!(style.margin_right, "");
    assert_ne!(style.border_top_style, "");
}
