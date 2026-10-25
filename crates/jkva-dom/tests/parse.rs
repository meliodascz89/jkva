//! Integration tests for the HTML tree builder.
//!
//! These exercise the public API only: parse a string, walk the arena, assert on
//! the shape. They are the safety net for the html5ever `TreeSink` wiring — a
//! broken link or a merged-in-the-wrong-place text node shows up here.

use jkva_dom::{Document, HTML_NAMESPACE, NodeId, NodeKind, QuirksMode, parse_document};

/// Returns the local name of an element node, or the kind for anything else.
fn describe(document: &Document, id: NodeId) -> String {
    match document.kind(id) {
        NodeKind::Document => "#document".to_string(),
        NodeKind::Element(element) => element.name().to_string(),
        NodeKind::Text(text) => format!("#text({text})"),
        NodeKind::Comment(text) => format!("#comment({text})"),
    }
}

/// Returns the child elements of a node, in tree order.
fn child_elements(document: &Document, id: NodeId) -> Vec<String> {
    document
        .children(id)
        .filter(|child| document.node(*child).is_element())
        .map(|child| describe(document, child))
        .collect()
}

/// Finds the first element with the given local name in document order.
fn first_element(document: &Document, name: &str) -> NodeId {
    document
        .descendants(document.root())
        .find(|id| {
            document
                .element(*id)
                .is_some_and(|element| element.name() == name)
        })
        .unwrap_or_else(|| panic!("no <{name}> in document"))
}

#[test]
fn implies_html_head_and_body() {
    let document = parse_document("<p>hi");
    let root = document.root();
    assert!(matches!(document.kind(root), NodeKind::Document));
    assert_eq!(document.children(root).count(), 1);

    let html = document.first_child(root).expect("implied <html>");
    assert_eq!(document.parent(html), Some(root));
    assert_eq!(describe(&document, html), "html");
    assert_eq!(child_elements(&document, html), vec!["head", "body"]);

    let head = document.first_child(html).expect("implied <head>");
    let body = document.next_sibling(head).expect("implied <body>");
    assert_eq!(child_elements(&document, head), Vec::<String>::new());
    assert_eq!(child_elements(&document, body), vec!["p"]);
    assert_eq!(document.text_content(body), "hi");
}

#[test]
fn nested_elements_keep_sibling_order() {
    let document = parse_document("<div><span>a</span><em>b</em><span>c</span></div>");
    let div = first_element(&document, "div");
    assert_eq!(child_elements(&document, div), vec!["span", "em", "span"]);

    let children: Vec<_> = document.children(div).collect();
    assert_eq!(document.first_child(div), Some(children[0]));
    assert_eq!(document.last_child(div), Some(children[2]));
    assert_eq!(document.prev_sibling(children[1]), Some(children[0]));
    assert_eq!(document.next_sibling(children[1]), Some(children[2]));
    assert_eq!(document.text_content(div), "abc");

    // The middle child is <em>b</em>: one text child with the letter b.
    let em = children[1];
    assert_eq!(document.text_content(em), "b");
    assert_eq!(document.children(em).count(), 1);
}

#[test]
fn attributes_keep_quotes_and_decode_entities() {
    let document = parse_document(
        r#"<a href="/x?a=1&amp;b=2" title='He said "hi"' data-x=unquoted CLASS="hero">t</a>"#,
    );
    let anchor = first_element(&document, "a");
    assert_eq!(document.get_attribute(anchor, "href"), Some("/x?a=1&b=2"));
    assert_eq!(
        document.get_attribute(anchor, "title"),
        Some(r#"He said "hi""#)
    );
    assert_eq!(document.get_attribute(anchor, "data-x"), Some("unquoted"));
    // The tokenizer lowercases HTML attribute names, so this is `class`.
    assert_eq!(document.get_attribute(anchor, "class"), Some("hero"));
    assert_eq!(document.get_attribute(anchor, "CLASS"), None);

    let element = document.element(anchor).expect("element");
    let names: Vec<_> = element.attributes().iter().map(|(name, _)| name).collect();
    assert_eq!(names, vec!["href", "title", "data-x", "class"]);
}

#[test]
fn text_nodes_are_merged_and_text_content_concatenates() {
    let document = parse_document("<div>a<b>bold</b>c</div>");
    let div = first_element(&document, "div");
    let kinds: Vec<_> = document
        .children(div)
        .map(|child| describe(&document, child))
        .collect();
    assert_eq!(
        kinds,
        vec![
            "#text(a)".to_string(),
            "b".to_string(),
            "#text(c)".to_string()
        ]
    );
    assert_eq!(document.text_content(div), "aboldc");

    // Entity decoding happens before the text reaches the tree.
    let entities = parse_document("<p>&lt;&amp;&gt;&#65;</p>");
    assert_eq!(entities.text_content(entities.root()), "<&>A");
}

#[test]
fn comments_and_doctype_are_preserved() {
    let document = parse_document("<!DOCTYPE html><div><!-- note --></div>");
    let doctype = document.doctype().expect("doctype");
    assert_eq!(doctype.name(), "html");
    assert_eq!(doctype.public_id(), "");
    assert_eq!(doctype.system_id(), "");

    let div = first_element(&document, "div");
    let comment = document.first_child(div).expect("comment child");
    match document.kind(comment) {
        NodeKind::Comment(text) => assert_eq!(text, " note "),
        other => panic!("expected a comment, got {other:?}"),
    }
    // Comments are not text content.
    assert_eq!(document.text_content(div), "");
}

#[test]
fn malformed_html_is_recovered() {
    // Unclosed <li> end tags, unclosed <ul>, unclosed <span>: the parser repairs
    // all three and still yields a well-formed tree.
    let document = parse_document("<ul><li>one<li>two</ul><div><span>dangling</div>");
    let list = first_element(&document, "ul");
    assert_eq!(child_elements(&document, list), vec!["li", "li"]);
    let items: Vec<_> = document.children(list).collect();
    assert_eq!(document.text_content(items[0]), "one");
    assert_eq!(document.text_content(items[1]), "two");

    let div = first_element(&document, "div");
    assert_eq!(child_elements(&document, div), vec!["span"]);
    assert_eq!(document.parent(first_element(&document, "span")), Some(div));

    // A second <p> implicitly closes the first one.
    let paragraphs = parse_document("<p>a<p>b");
    let body = first_element(&paragraphs, "body");
    assert_eq!(child_elements(&paragraphs, body), vec!["p", "p"]);
    assert_eq!(paragraphs.text_content(body), "ab");
}

#[test]
fn template_contents_live_in_a_detached_fragment() {
    let document = parse_document("<template><p>inside</p></template>");
    let template = first_element(&document, "template");
    assert_eq!(document.children(template).count(), 0);

    let fragment = document
        .element(template)
        .and_then(|element| element.template_contents())
        .expect("template must own a contents fragment");
    assert_eq!(document.parent(fragment), None);
    assert!(matches!(document.kind(fragment), NodeKind::Document));
    assert_eq!(child_elements(&document, fragment), vec!["p"]);
    assert_eq!(document.text_content(fragment), "inside");

    // The fragment is not reachable from the root, so it is not text content of
    // the document.
    assert_eq!(document.text_content(document.root()), "");
    assert!(
        !document
            .descendants(document.root())
            .any(|id| id == fragment)
    );
}

#[test]
fn quirks_mode_follows_the_doctype() {
    assert_eq!(
        parse_document("<!DOCTYPE html><p>x").quirks_mode(),
        QuirksMode::NoQuirks
    );
    // No doctype at all means full quirks mode.
    assert_eq!(parse_document("<p>x").quirks_mode(), QuirksMode::Quirks);
    // A legacy HTML 4.01 Transitional doctype is almost standards mode.
    let legacy = parse_document(
        r#"<!DOCTYPE HTML PUBLIC "-//W3C//DTD HTML 4.01 Transitional//EN" "http://www.w3.org/TR/html4/loose.dtd"><p>x"#,
    );
    assert_eq!(legacy.quirks_mode(), QuirksMode::LimitedQuirks);
}

#[test]
fn parser_diagnostics_are_recorded() {
    let clean = parse_document("<!DOCTYPE html><p>fine</p>");
    assert!(
        clean.parse_errors().is_empty(),
        "well-formed markup must not report diagnostics: {:?}",
        clean.parse_errors().retained()
    );

    let broken = parse_document("<div><span></div></span><p");
    let errors = broken.parse_errors();
    assert!(!errors.is_empty(), "broken markup must report diagnostics");
    assert!(errors.total_count() >= errors.retained().len());
    assert!(
        errors.retained().iter().all(|message| !message.is_empty()),
        "messages must not be empty"
    );
}

#[test]
fn svg_elements_keep_their_namespace() {
    let document = parse_document("<svg><circle cx='1'/></svg>");
    let svg = first_element(&document, "svg");
    let element = document.element(svg).expect("svg element");
    assert_eq!(element.namespace(), "http://www.w3.org/2000/svg");
    assert!(!element.is_html());

    let circle = first_element(&document, "circle");
    let circle = document.element(circle).expect("circle element");
    assert_eq!(circle.namespace(), "http://www.w3.org/2000/svg");
    assert_eq!(circle.attribute("cx"), Some("1"));

    // Ordinary HTML elements are in the HTML namespace instead.
    let document = parse_document("<div></div>");
    let div = document
        .element(first_element(&document, "div"))
        .expect("div");
    assert_eq!(div.namespace(), HTML_NAMESPACE);
    assert!(div.is_html());
}

#[test]
fn head_content_goes_to_head() {
    let document = parse_document("<title>T</title><meta charset=utf-8><p>x");
    let head = first_element(&document, "head");
    assert_eq!(child_elements(&document, head), vec!["title", "meta"]);
    assert_eq!(document.text_content(head), "T");
    let body = first_element(&document, "body");
    assert_eq!(child_elements(&document, body), vec!["p"]);
}

#[test]
fn table_markup_is_foster_parented() {
    let document = parse_document("<table><div>loose</div><tr><td>cell</td></tr></table>");
    let table = first_element(&document, "table");
    // The stray <div> is moved before the table by foster parenting; the table
    // itself keeps only its row.
    assert_eq!(child_elements(&document, table), vec!["tbody"]);
    let body = first_element(&document, "body");
    assert_eq!(child_elements(&document, body), vec!["div", "table"]);
    assert_eq!(
        document.text_content(first_element(&document, "div")),
        "loose"
    );
    assert_eq!(document.text_content(document.root()), "loosecell");
}

/// Asserts that every forward link of the tree has a matching reverse link.
///
/// This is the invariant the arena exists for: if the tree builder ever wired a
/// node incorrectly, one of these checks fails.
fn assert_links_are_consistent(document: &Document) {
    let mut visited = 0;
    for id in document.descendants(document.root()) {
        visited += 1;
        if let Some(next) = document.next_sibling(id) {
            assert_eq!(
                document.prev_sibling(next),
                Some(id),
                "next/prev mismatch after {id:?}"
            );
        }
        if let Some(previous) = document.prev_sibling(id) {
            assert_eq!(
                document.next_sibling(previous),
                Some(id),
                "prev/next mismatch before {id:?}"
            );
        }
        if let Some(first) = document.first_child(id) {
            assert_eq!(document.parent(first), Some(id));
            assert_eq!(document.prev_sibling(first), None);
        }
        if let Some(last) = document.last_child(id) {
            assert_eq!(document.parent(last), Some(id));
            assert_eq!(document.next_sibling(last), None);
        }
        assert_eq!(
            document.first_child(id).is_some(),
            document.last_child(id).is_some()
        );
    }
    assert!(visited > 0, "the root must be visited");
}

#[test]
fn adversarial_input_does_not_panic_and_keeps_links_consistent() {
    // Deeply nested markup, an unterminated paragraph and an unterminated comment:
    // the parser must recover and the links must still agree in both directions.
    let mut html = "<div>".repeat(200);
    html.push_str("<p>x<!-- unterminated comment");
    let document = parse_document(&html);

    assert_links_are_consistent(&document);
    assert!(
        document.node_count() > 200,
        "the nested divs must survive parsing"
    );
    assert!(document.text_content(document.root()).contains('x'));
}

#[test]
fn explicit_html_body_and_templates_in_tables_stay_consistent() {
    // An explicit <html>/<body> pair, a template foster-parented inside a table and
    // stray end tags: all parser recovery paths in one document.
    let document = parse_document(
        "<html><head></head><body></body></html><table><template><tr><td>t</td></tr></template></table></div>",
    );
    assert_links_are_consistent(&document);
    let html = document.first_child(document.root()).expect("<html>");
    assert_eq!(describe(&document, html), "html");
    assert_eq!(child_elements(&document, html), vec!["head", "body"]);
    assert_eq!(document.children(document.root()).count(), 1);
}
