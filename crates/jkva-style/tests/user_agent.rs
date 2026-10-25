//! The built-in user-agent stylesheet and the cascade origin ordering.

mod common;

use common::{by_id, by_tag, engine_of, parse_html, style_of};
use jkva_style::{DisplayType, StyleEngine, Stylesheet, ua::USER_AGENT_CSS};

/// A document with one element of every UA-sheet category.
///
/// `<title>` comes first so the parser builds an implied `<head>` for it, then
/// the rest lands in `<body>`.
fn fixture() -> jkva_dom::Document {
    parse_html(
        r#"<title>t</title>
           <div id="d"></div>
           <span id="s"></span>
           <h1 id="h1">heading</h1>
           <p id="p">paragraph</p>
           <ul id="u"><li id="li">item</li></ul>
           <b id="b">bold</b>
           <pre id="pre">code</pre>"#,
    )
}

#[test]
fn a_fresh_engine_loads_exactly_the_user_agent_sheet() {
    let engine = StyleEngine::new();
    assert_eq!(engine.sheets().len(), 1);
    assert_eq!(
        engine.sheets()[0],
        Stylesheet::parse(USER_AGENT_CSS).expect("the UA sheet parses")
    );

    // An engine built for author-only testing has nothing at all.
    assert!(StyleEngine::without_user_agent().sheets().is_empty());
}

#[test]
fn the_user_agent_sheet_supplies_box_types_and_defaults() {
    let document = fixture();
    let styles = StyleEngine::new().compute_styles(&document);

    // Block-level.
    for id in ["d", "h1", "p", "u", "li", "pre"] {
        assert_eq!(
            style_of(&document, &styles, id).display,
            DisplayType::Block,
            "#{id} should be a block"
        );
    }
    // `li` is block rather than `list-item` until marker boxes exist; the UA
    // sheet documents this simplification.
    assert_eq!(
        style_of(&document, &styles, "li").display,
        DisplayType::Block
    );

    // Inline-level.
    assert_eq!(
        style_of(&document, &styles, "s").display,
        DisplayType::Inline
    );

    // Non-rendered elements.
    let head = by_tag(&document, "head");
    let title = by_tag(&document, "title");
    assert_eq!(styles.get(head).unwrap().display, DisplayType::None);
    assert_eq!(styles.get(title).unwrap().display, DisplayType::None);
}

#[test]
fn the_user_agent_sheet_supplies_the_typographic_defaults() {
    let document = fixture();
    let styles = StyleEngine::new().compute_styles(&document);

    // `body { margin: 8px }`.
    let body = by_tag(&document, "body");
    let body_style = styles.get(body).unwrap();
    assert_eq!(body_style.margin_top, "8px");
    assert_eq!(body_style.margin_left, "8px");

    // `p { margin: 1em 0 }` — the shorthand expands to four sides.
    let paragraph = style_of(&document, &styles, "p");
    assert_eq!(paragraph.margin_top, "1em");
    assert_eq!(paragraph.margin_right, "0");

    // The heading scale.
    let heading = style_of(&document, &styles, "h1");
    assert_eq!(heading.font_size, "2em");
    assert_eq!(heading.margin_top, "0.67em");
    assert_eq!(heading.margin_left, "0");
    assert_eq!(heading.font_weight, "bold");

    // `ul { margin: 1em 0; padding-left: 40px }`.
    let list = style_of(&document, &styles, "u");
    assert_eq!(list.margin_top, "1em");
    assert_eq!(list.padding_left, "40px");

    // The monospace group and bold text.
    assert_eq!(style_of(&document, &styles, "pre").font_family, "monospace");
    assert_eq!(style_of(&document, &styles, "b").font_weight, "bold");

    // Inherited properties flow from the UA sheet into children.
    let item = style_of(&document, &styles, "li");
    assert_eq!(item.font_size, "16px", "the initial font size inherits");
    assert_eq!(item.color, "black");
}

#[test]
fn an_author_rule_overrides_the_user_agent_at_equal_specificity() {
    let document = parse_html("<p id=p>x</p>");

    // Baseline: the UA sheet alone.
    let ua_style = style_of(
        &document,
        &StyleEngine::new().compute_styles(&document),
        "p",
    );
    assert_eq!(ua_style.display, DisplayType::Block);
    assert_eq!(ua_style.margin_top, "1em");

    // Equal specificity (`p` is `(0, 0, 1)` in both origins): the author wins.
    let engine = engine_of("p { display: inline-block; margin: 3px }");
    let style = style_of(&document, &engine.compute_styles(&document), "p");
    assert_eq!(style.display, DisplayType::InlineBlock);
    assert_eq!(style.margin_top, "3px");
    assert_eq!(style.margin_left, "3px");
}

#[test]
fn author_important_beats_user_agent_normal() {
    let document = parse_html("<p id=p>x</p>");
    let mut engine = StyleEngine::without_user_agent();
    engine.add_user_agent_stylesheet(Stylesheet::parse("p { color: red }").unwrap());
    engine.add_stylesheet(Stylesheet::parse("p { color: blue !important }").unwrap());

    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "p").color,
        "blue"
    );
}

#[test]
fn user_agent_important_beats_author_important() {
    // The only rank above an author `!important`: a UA `!important`.
    let document = parse_html("<p id=p>x</p>");

    let mut engine = StyleEngine::without_user_agent();
    engine.add_user_agent_stylesheet(Stylesheet::parse("p { color: red !important }").unwrap());
    engine.add_stylesheet(Stylesheet::parse("p { color: blue !important }").unwrap());
    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "p").color,
        "red"
    );

    // ...even when the author declaration is far more specific.
    let mut engine = StyleEngine::without_user_agent();
    engine.add_user_agent_stylesheet(Stylesheet::parse("p { color: red !important }").unwrap());
    engine.add_stylesheet(Stylesheet::parse("#p { color: blue !important }").unwrap());
    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "p").color,
        "red"
    );

    // A UA `!important` also beats an author normal declaration, of course.
    let mut engine = StyleEngine::without_user_agent();
    engine.add_user_agent_stylesheet(Stylesheet::parse("p { color: red !important }").unwrap());
    engine.add_stylesheet(Stylesheet::parse("p { color: blue }").unwrap());
    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "p").color,
        "red"
    );
}

#[test]
fn origin_outranks_specificity() {
    let document = parse_html("<p id=p>x</p>");

    // Normal declarations: UA `#p` (1,0,0) loses to author `p` (0,0,1), because
    // the origin step runs before specificity.
    let mut engine = StyleEngine::without_user_agent();
    engine.add_user_agent_stylesheet(Stylesheet::parse("#p { color: red }").unwrap());
    engine.add_stylesheet(Stylesheet::parse("p { color: blue }").unwrap());
    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "p").color,
        "blue",
        "the author origin wins before specificity is consulted"
    );

    // Important declarations reverse the origin order: UA `p` (0,0,1) beats
    // author `#p` (1,0,0) for the same reason.
    let mut engine = StyleEngine::without_user_agent();
    engine.add_user_agent_stylesheet(Stylesheet::parse("p { color: red !important }").unwrap());
    engine.add_stylesheet(Stylesheet::parse("#p { color: blue !important }").unwrap());
    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "p").color,
        "red",
        "the UA origin wins before specificity is consulted"
    );
}

#[test]
fn within_one_origin_the_usual_rules_still_apply() {
    // Source order and specificity are unaffected by the origin machinery.
    let document = parse_html(r#"<p id="p" class="note">x</p>"#);

    let mut engine = StyleEngine::without_user_agent();
    engine.add_user_agent_stylesheet(Stylesheet::parse("p { color: red }").unwrap());
    engine.add_user_agent_stylesheet(Stylesheet::parse("#p { color: green }").unwrap());
    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "p").color,
        "green",
        "specificity decides inside the UA origin"
    );

    let mut engine = StyleEngine::without_user_agent();
    engine.add_user_agent_stylesheet(Stylesheet::parse("p { color: red }").unwrap());
    engine.add_user_agent_stylesheet(Stylesheet::parse("p { color: green }").unwrap());
    assert_eq!(
        style_of(&document, &engine.compute_styles(&document), "p").color,
        "green",
        "source order decides inside the UA origin"
    );
}

#[test]
fn user_agent_declarations_are_inherited_like_any_other() {
    // The UA heading scale inherits into children, and an author rule can read it
    // back through `inherit`.
    let document = parse_html(r#"<h1 id="h"><span id="s">x</span></h1>"#);

    let engine = engine_of("#s { font-size: inherit; color: inherit }");
    let styles = engine.compute_styles(&document);
    let heading = style_of(&document, &styles, "h");
    let span = style_of(&document, &styles, "s");

    assert_eq!(heading.font_size, "2em");
    assert_eq!(span.font_size, "2em");
    assert_eq!(span.color, "black");
}

#[test]
fn the_user_agent_sheet_does_not_style_unknown_elements() {
    let document = parse_html(r#"<x-widget id="w">x</x-widget>"#);
    let styles = StyleEngine::new().compute_styles(&document);
    let widget = by_id(&document, "w");

    assert_eq!(
        styles.get(widget).map(|style| style.display),
        Some(DisplayType::Inline),
        "an unknown element keeps the initial display"
    );
}
