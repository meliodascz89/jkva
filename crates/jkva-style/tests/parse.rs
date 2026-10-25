//! Integration tests for the public API of `jkva-style`.
//!
//! These tests go through the crate's public surface only, which doubles as a
//! check that the public surface is actually usable from outside.

use jkva_style::{
    AttributeOperator, Combinator, ErrorKind, PseudoClass, SimpleSelector, Specificity, Stylesheet,
    parse_selector_list,
};

/// Parses a stylesheet that must not produce any fatal error.
fn parse(css: &str) -> Stylesheet {
    Stylesheet::parse(css).expect("stylesheet should parse")
}

#[test]
fn simple_rule() {
    let sheet = parse("div { color: red }");

    assert_eq!(sheet.rules().len(), 1);
    let rule = &sheet.rules()[0];
    assert_eq!(rule.selectors.len(), 1);
    assert_eq!(rule.selectors[0].to_string(), "div");
    assert_eq!(rule.selectors[0].specificity(), Specificity::new(0, 0, 1));

    assert_eq!(rule.declarations.len(), 1);
    assert_eq!(rule.declarations[0].name, "color");
    assert_eq!(rule.declarations[0].value, "red");
    assert!(!rule.declarations[0].important);
    assert!(sheet.warnings().is_empty());
}

#[test]
fn selector_list_is_split_on_commas() {
    let sheet = parse("h1, .title , #main { color: red }");
    let rule = &sheet.rules()[0];

    assert_eq!(rule.selectors.len(), 3);
    assert_eq!(rule.selectors[0].to_string(), "h1");
    assert_eq!(rule.selectors[1].to_string(), ".title");
    assert_eq!(rule.selectors[2].to_string(), "#main");

    let specificities: Vec<_> = rule
        .selector_specificities()
        .map(|(_, specificity)| specificity)
        .collect();
    assert_eq!(
        specificities,
        vec![
            Specificity::new(0, 0, 1),
            Specificity::new(0, 1, 0),
            Specificity::new(1, 0, 0),
        ]
    );
}

#[test]
fn important_is_detected_and_stripped_from_the_value() {
    let sheet = parse("p { color: red !important; margin: 0 }");
    let declarations = &sheet.rules()[0].declarations;

    assert_eq!(declarations[0].value, "red");
    assert!(declarations[0].important);

    assert_eq!(declarations[1].value, "0");
    assert!(!declarations[1].important);
}

#[test]
fn important_is_ascii_case_insensitive() {
    let sheet = parse("p { color: red !IMPORTANT }");
    assert!(sheet.rules()[0].declarations[0].important);
}

#[test]
fn a_single_bang_does_not_break_the_following_declarations() {
    // `!ie` is the classic IE hack: the declaration is invalid, the rest of the
    // block must survive.
    let sheet = parse("p { color: red !ie; margin: 1px }");
    let declarations = &sheet.rules()[0].declarations;

    assert_eq!(declarations.len(), 1);
    assert_eq!(declarations[0].name, "margin");
    assert_eq!(sheet.warnings().len(), 1);
    assert_eq!(sheet.warnings()[0].kind, ErrorKind::InvalidDeclaration);
}

#[test]
fn comments_are_ignored_everywhere() {
    let css = "
        /* a leading comment */
        div /* after the selector */, p {
            /* before a declaration */
            color /* inside */ : red /* after the value */ ;
            /* after the declaration */
        }
        /* a trailing comment */
    ";
    let sheet = parse(css);

    assert_eq!(sheet.rules().len(), 1);
    let rule = &sheet.rules()[0];
    assert_eq!(rule.selectors.len(), 2);
    assert_eq!(rule.selectors[0].to_string(), "div");
    assert_eq!(rule.selectors[1].to_string(), "p");
    assert_eq!(rule.declarations.len(), 1);
    assert_eq!(rule.declarations[0].name, "color");
    assert_eq!(rule.declarations[0].value, "red");
    assert!(sheet.warnings().is_empty());
}

#[test]
fn a_comment_is_not_whitespace_between_compound_selectors() {
    // CSS removes comments without inserting a space, so `div/**/p` is two
    // adjacent type selectors and therefore invalid.
    let sheet = parse("div/**/p { color: red }");
    assert!(sheet.is_empty());
    assert_eq!(sheet.warnings().len(), 1);
}

#[test]
fn unknown_at_rules_are_recorded_and_skipped() {
    let css = "
        @media (min-width: 600px) {
            .hidden { display: none }
        }
        @import url(theme.css);
        div { color: red }
    ";
    let sheet = parse(css);

    assert_eq!(sheet.rules().len(), 1);
    assert_eq!(sheet.rules()[0].declarations[0].name, "color");

    assert_eq!(sheet.at_rules().len(), 2);
    assert_eq!(sheet.at_rules()[0].name, "media");
    assert_eq!(sheet.at_rules()[0].prelude, "(min-width: 600px)");
    assert_eq!(sheet.at_rules()[1].name, "import");
    assert_eq!(sheet.at_rules()[1].prelude, "url(theme.css)");

    // Skipping an at-rule is not an error.
    assert!(sheet.warnings().is_empty());
}

#[test]
fn malformed_declarations_are_dropped_and_reported() {
    let css = "div { color red; background: blue; width: ; }";
    let sheet = parse(css);

    let rule = &sheet.rules()[0];
    let names: Vec<_> = rule
        .declarations
        .iter()
        .map(|declaration| declaration.name.as_str())
        .collect();
    assert_eq!(names, vec!["background"]);

    assert_eq!(sheet.warnings().len(), 2);
    assert!(
        sheet
            .warnings()
            .iter()
            .all(|warning| warning.kind == ErrorKind::InvalidDeclaration),
        "unexpected warning kinds: {:?}",
        sheet.warnings()
    );
    assert!(
        sheet.warnings()[0].source.contains("color red"),
        "warning should quote the offending text, got {:?}",
        sheet.warnings()[0].source
    );
    assert!(sheet.warnings()[0].position.line >= 1);
}

#[test]
fn a_rule_with_an_invalid_selector_is_dropped_but_parsing_continues() {
    let css = "div > { color: red } p { color: blue }";
    let sheet = parse(css);

    assert_eq!(sheet.rules().len(), 1);
    assert_eq!(sheet.rules()[0].selectors[0].to_string(), "p");
    assert_eq!(sheet.rules()[0].declarations[0].value, "blue");
    assert_eq!(sheet.warnings().len(), 1);
    assert!(
        matches!(
            sheet.warnings()[0].kind,
            ErrorKind::InvalidSelector
                | ErrorKind::InvalidRule
                | ErrorKind::UnexpectedToken
                | ErrorKind::UnexpectedEndOfInput
        ),
        "unexpected warning kind {:?}",
        sheet.warnings()[0].kind
    );
}

#[test]
fn specificity_vector() {
    let cases: &[(&str, Specificity)] = &[
        ("*", Specificity::new(0, 0, 0)),
        ("div", Specificity::new(0, 0, 1)),
        (".note", Specificity::new(0, 1, 0)),
        ("#main", Specificity::new(1, 0, 0)),
        ("[type]", Specificity::new(0, 1, 0)),
        (":hover", Specificity::new(0, 1, 0)),
        ("::before", Specificity::new(0, 0, 1)),
        (":before", Specificity::new(0, 0, 1)),
        ("div.note", Specificity::new(0, 1, 1)),
        ("#main .note div", Specificity::new(1, 1, 1)),
        ("div > p + span ~ a", Specificity::new(0, 0, 4)),
        ("div p", Specificity::new(0, 0, 2)),
        (":where(#main) div", Specificity::new(0, 0, 1)),
        (":is(.a, #b)", Specificity::new(1, 0, 0)),
        (":not(div, .x)", Specificity::new(0, 1, 0)),
        (":has(> img)", Specificity::new(0, 0, 1)),
        ("a:has(> .icon)", Specificity::new(0, 1, 1)),
        ("::selection", Specificity::new(0, 0, 1)),
        ("a:hover::after", Specificity::new(0, 1, 2)),
        ("*|div", Specificity::new(0, 0, 1)),
    ];

    for (css, expected) in cases {
        let selectors = parse_selector_list(css).unwrap_or_else(|error| {
            panic!("{css} should parse: {error}");
        });
        assert_eq!(selectors.len(), 1, "{css}");
        assert_eq!(
            selectors[0].specificity(),
            *expected,
            "wrong specificity for {css}"
        );
    }
}

#[test]
fn specificity_ordering_is_lexicographic() {
    let id = Specificity::new(1, 0, 0);
    let classes = Specificity::new(0, 2, 0);
    let types = Specificity::new(0, 0, 9);
    assert!(id > classes);
    assert!(classes > types);
    assert_eq!(Specificity::new(1, 0, 0).max(classes), id);
}

#[test]
fn combinators_are_modelled_explicitly() {
    let selectors = parse_selector_list("div > p + span ~ a b").unwrap();
    let components = selectors[0].components();

    assert_eq!(components.len(), 5);
    assert_eq!(components[0].combinator, None);
    assert_eq!(components[1].combinator, Some(Combinator::Child));
    assert_eq!(components[2].combinator, Some(Combinator::NextSibling));
    assert_eq!(components[3].combinator, Some(Combinator::LaterSibling));
    assert_eq!(components[4].combinator, Some(Combinator::Descendant));
    assert_eq!(selectors[0].to_string(), "div > p + span ~ a b");
}

#[test]
fn attribute_selectors_are_modelled() {
    let selectors =
        parse_selector_list(r#"[disabled], [type="text"], [lang|=en i], [class~=a], [href^=http], [src$=".png"], [title*=x]"#)
            .unwrap();

    assert_eq!(selectors.len(), 7);

    let plain = selectors[0].components()[0].compound.simples[0].clone();
    let SimpleSelector::Attribute(attribute) = plain else {
        panic!("expected an attribute selector");
    };
    assert_eq!(attribute.name, "disabled");
    assert!(attribute.matcher.is_none());

    let quoted = selectors[1].components()[0].compound.simples[0].clone();
    let SimpleSelector::Attribute(attribute) = quoted else {
        panic!("expected an attribute selector");
    };
    assert_eq!(attribute.name, "type");
    let matcher = attribute.matcher.expect("matcher");
    assert_eq!(matcher.operator, AttributeOperator::Equals);
    assert_eq!(matcher.value, "text");

    let dash = selectors[2].components()[0].compound.simples[0].clone();
    let SimpleSelector::Attribute(attribute) = dash else {
        panic!("expected an attribute selector");
    };
    let matcher = attribute.matcher.expect("matcher");
    assert_eq!(matcher.operator, AttributeOperator::DashMatch);
    assert!(attribute.case_insensitive);
}

#[test]
fn pseudo_classes_and_elements_are_modelled() {
    let selectors = parse_selector_list("a:hover, li:nth-child(2n + 1), p::first-line").unwrap();

    let SimpleSelector::PseudoClass(PseudoClass::Other { name, argument }) =
        selectors[0].components()[0].compound.simples[1].clone()
    else {
        panic!("expected a pseudo-class");
    };
    assert_eq!(name, "hover");
    assert!(argument.is_none());

    let SimpleSelector::PseudoClass(PseudoClass::Other { name, argument }) =
        selectors[1].components()[0].compound.simples[1].clone()
    else {
        panic!("expected a pseudo-class");
    };
    assert_eq!(name, "nth-child");
    assert_eq!(argument.as_deref(), Some("2n + 1"));

    let SimpleSelector::PseudoElement(pseudo_element) =
        selectors[2].components()[0].compound.simples[1].clone()
    else {
        panic!("expected a pseudo-element");
    };
    assert_eq!(pseudo_element.name, "first-line");
    assert_eq!(selectors[2].to_string(), "p::first-line");
}

#[test]
fn selector_list_pseudo_classes_nest_their_arguments() {
    let selectors = parse_selector_list(":is(.a, .b) :not(#c)").unwrap();
    let components = selectors[0].components();

    let SimpleSelector::PseudoClass(PseudoClass::Is(arguments)) =
        components[0].compound.simples[0].clone()
    else {
        panic!("expected :is()");
    };
    assert_eq!(arguments.len(), 2);
    assert_eq!(arguments[0].to_string(), ".a");
    assert_eq!(arguments[1].to_string(), ".b");

    assert_eq!(components.len(), 2);
    assert_eq!(components[1].combinator, Some(Combinator::Descendant));
    let SimpleSelector::PseudoClass(PseudoClass::Not(arguments)) =
        components[1].compound.simples[0].clone()
    else {
        panic!("expected :not()");
    };
    assert_eq!(arguments.len(), 1);
    assert_eq!(arguments[0].to_string(), "#c");
}

#[test]
fn has_takes_relative_selectors_with_a_leading_combinator() {
    let selectors = parse_selector_list("article:has(> img)").unwrap();
    let SimpleSelector::PseudoClass(PseudoClass::Has(arguments)) =
        selectors[0].components()[0].compound.simples[1].clone()
    else {
        panic!("expected :has()");
    };

    assert_eq!(arguments.len(), 1);
    assert_eq!(arguments[0].leading_combinator(), Some(Combinator::Child));
    assert_eq!(arguments[0].to_string(), "> img");

    let selectors = parse_selector_list("article:has(img)").unwrap();
    let SimpleSelector::PseudoClass(PseudoClass::Has(arguments)) =
        selectors[0].components()[0].compound.simples[1].clone()
    else {
        panic!("expected :has()");
    };
    assert_eq!(arguments[0].leading_combinator(), None);
}

#[test]
fn declaration_values_are_kept_verbatim() {
    let sheet = parse("div { margin: 0   auto; background: url(a.png) rgb(1, 2, 3) }");
    let declarations = &sheet.rules()[0].declarations;

    assert_eq!(declarations[0].value, "0   auto");
    assert_eq!(declarations[1].value, "url(a.png) rgb(1, 2, 3)");
}

#[test]
fn declaration_order_and_duplicates_are_preserved() {
    let sheet = parse("div { color: red; color: blue; width: 1px }");
    let names: Vec<_> = sheet.rules()[0]
        .declarations
        .iter()
        .map(|declaration| declaration.value.as_str())
        .collect();
    assert_eq!(names, vec!["red", "blue", "1px"]);
}

#[test]
fn property_names_are_lowercased_but_custom_properties_are_not() {
    let sheet = parse("div { COLOR: red; --BrandColor: blue }");
    let declarations = &sheet.rules()[0].declarations;

    assert_eq!(declarations[0].name, "color");
    assert!(!declarations[0].is_custom_property());

    assert_eq!(declarations[1].name, "--BrandColor");
    assert!(declarations[1].is_custom_property());
}

#[test]
fn a_bang_inside_a_string_is_not_important() {
    let sheet = parse(r#"div { content: "a!b" }"#);
    assert_eq!(sheet.rules()[0].declarations[0].value, "\"a!b\"");
    assert!(!sheet.rules()[0].declarations[0].important);
}

#[test]
fn a_documented_limitation_bang_in_a_custom_property_is_rejected() {
    // The CSS spec allows arbitrary `!` in custom property values; this crate
    // treats a top-level `!` as an attempted `!important`. Pinned here so that
    // the limitation is visible and cannot regress silently.
    let sheet = parse("div { --x: a!b }");
    assert!(sheet.rules()[0].declarations.is_empty());
    assert_eq!(sheet.warnings()[0].kind, ErrorKind::InvalidDeclaration);
}

#[test]
fn nesting_is_not_supported_yet() {
    // CSS Nesting is explicitly out of scope for this crate version; the nested
    // rule must be dropped without breaking the outer one.
    let sheet = parse("div { color: red; & .child { color: blue } width: 1px }");

    let names: Vec<_> = sheet.rules()[0]
        .declarations
        .iter()
        .map(|declaration| declaration.name.as_str())
        .collect();
    assert_eq!(names, vec!["color"]);
    assert!(!sheet.warnings().is_empty());
}

#[test]
fn parse_selector_list_accepts_valid_input_and_rejects_invalid_input() {
    let selectors = parse_selector_list("main > .title, #hero").unwrap();
    assert_eq!(selectors.len(), 2);
    assert_eq!(selectors[0].to_string(), "main > .title");
    assert_eq!(selectors[1].to_string(), "#hero");

    for invalid in ["", "div >", "div,", ".", "div p {", ":is()", "a::"] {
        assert!(
            parse_selector_list(invalid).is_err(),
            "{invalid:?} should be rejected"
        );
    }
}

#[test]
fn selector_display_round_trips_through_the_parser() {
    for css in [
        "a",
        "*",
        "div.note#x",
        "div > p",
        "a b c",
        "main > article .title:hover::before",
        ":is(.a, #b)",
        ":where(div) p",
        "a:has(> img)",
        "svg|rect",
        "*|*",
        "[type=\"text\" i]",
        "li:nth-child(2n+1)",
    ] {
        let selectors = parse_selector_list(css).unwrap_or_else(|error| panic!("{css}: {error}"));
        assert_eq!(selectors.len(), 1, "{css}");
        let round_tripped = selectors[0].to_string();
        let reparsed = parse_selector_list(&round_tripped)
            .unwrap_or_else(|error| panic!("{round_tripped} should re-parse: {error}"));
        assert_eq!(reparsed, selectors, "{css} did not round-trip");
    }
}

#[test]
fn source_order_of_rules_is_preserved() {
    let sheet = parse("a { color: red } b { color: green } c { color: blue }");
    let selectors: Vec<_> = sheet
        .rules()
        .iter()
        .map(|rule| rule.selectors[0].to_string())
        .collect();
    assert_eq!(selectors, vec!["a", "b", "c"]);
}

#[test]
fn an_empty_stylesheet_parses_to_nothing() {
    for css in ["", "   \n\t ", "/* only a comment */"] {
        let sheet = parse(css);
        assert!(sheet.is_empty());
        assert!(sheet.at_rules().is_empty());
        assert!(sheet.warnings().is_empty());
    }
}

#[test]
fn deeply_nested_input_is_a_fatal_error() {
    // Nested selector blocks are the only construct the parser recurses into.
    let depth = 200;
    let selector = format!(
        "{}div{}{{ color: red }}",
        ":is(".repeat(depth),
        ")".repeat(depth)
    );
    let error = Stylesheet::parse(&selector).expect_err("deep nesting should be rejected");
    assert_eq!(error.kind(), ErrorKind::TooDeeplyNested);
    assert!(error.to_string().contains(':'));

    // The same limit protects the standalone selector parser.
    let bare = format!("{}div{}", ":is(".repeat(depth), ")".repeat(depth));
    assert!(parse_selector_list(&bare).is_err());
}

#[test]
fn whitespace_around_combinators_is_flexible() {
    for css in ["div>p", "div >p", "div> p", "div   >   p", "div\n>\np"] {
        let selectors = parse_selector_list(css).unwrap_or_else(|error| panic!("{css}: {error}"));
        assert_eq!(selectors[0].to_string(), "div > p", "{css}");
    }
}

#[test]
fn whitespace_after_a_prefix_character_is_rejected() {
    // `.`, `:` and `|` must be glued to the name that follows them.
    for css in ["div. a", "div : hover", "svg| rect"] {
        assert!(
            parse_selector_list(css).is_err(),
            "{css:?} should be rejected"
        );
    }
}

#[test]
fn id_selectors_require_a_valid_identifier() {
    assert!(parse_selector_list("#main").is_ok());
    // `#1` tokenises as an unrestricted hash, which is not an ID selector.
    assert!(parse_selector_list("#1").is_err());
}

#[test]
fn an_invalid_pseudo_class_argument_invalidates_the_whole_selector() {
    assert!(parse_selector_list(":is(.a, div >)").is_err());
    assert!(parse_selector_list(":is()").is_err());

    let sheet = parse(":is(.a, div >) { color: red } p { color: blue }");
    assert_eq!(sheet.rules().len(), 1);
    assert_eq!(sheet.rules()[0].selectors[0].to_string(), "p");
    assert_eq!(sheet.warnings().len(), 1);
}

#[test]
fn a_top_level_custom_property_lookalike_rule_is_reported() {
    // `--x: 1` at the top level is not a valid style rule; cssparser refuses to
    // treat it as one and reports it, which must not be fatal.
    let sheet = parse("--x: 1;");
    assert!(sheet.is_empty());
    assert_eq!(sheet.warnings().len(), 1);
}

#[test]
fn every_rule_keeps_its_own_declarations() {
    let sheet = parse("a { color: red } b { color: blue; width: 1px }");
    assert_eq!(sheet.rules()[0].declarations.len(), 1);
    assert_eq!(sheet.rules()[1].declarations.len(), 2);
    assert_eq!(sheet.rules()[1].declarations[1].name, "width");
}

#[test]
fn specificity_of_a_complex_selector_is_the_sum_of_its_parts() {
    let selectors = parse_selector_list("main#app > section.hero a[href]:hover::after").unwrap();
    assert_eq!(selectors[0].specificity(), Specificity::new(1, 3, 4));

    // Each compound selector contributes independently.
    let compound = &selectors[0].components()[2].compound;
    assert_eq!(compound.specificity(), Specificity::new(0, 2, 2));
    assert_eq!(compound.len(), 4);
    assert!(!compound.is_empty());
}

#[test]
fn to_css_string_serialises_every_rule_in_order() {
    let sheet = parse("a { color: red } b { width: 1px !important }");
    assert_eq!(
        sheet.to_css_string(),
        "a { color: red; }\nb { width: 1px !important; }"
    );
}

#[test]
fn a_stylesheet_can_be_iterated_by_reference() {
    let sheet = parse("a { color: red } b { color: blue }");

    let mut visited = 0;
    for rule in &sheet {
        assert_eq!(rule.declarations.len(), 1);
        visited += 1;
    }
    assert_eq!(visited, 2);
    assert_eq!(sheet.iter().count(), 2);
}

#[test]
fn stylesheet_parses_from_str() {
    use std::str::FromStr;
    let sheet = Stylesheet::from_str("a { color: red }").unwrap();
    assert_eq!(sheet.rules().len(), 1);
}
