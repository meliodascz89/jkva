//! The cssparser glue: source text in, [`Stylesheet`] out.
//!
//! Everything in this module is private plumbing. The public surface is
//! [`Stylesheet::parse`](crate::Stylesheet::parse) and
//! [`parse_selector_list`](crate::parse_selector_list).
//!
//! cssparser gives us a tokenizer and the rule/declaration parsing framework
//! (see <https://drafts.csswg.org/css-syntax/>), but no selector grammar, so the
//! selector parser below is ours. It is written directly against
//! [`cssparser::Token`] rather than against a general selector crate, because
//! we want a selector model shaped for JKva's own cascade instead of Servo's
//! `Element` trait.
//!
//! Error recovery is delegated to the framework: cssparser consumes the
//! remainder of a malformed rule or declaration before reporting the error, so
//! every error we see is already recoverable and parsing always continues with
//! the next construct.

use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError as CssParseError, Parser, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser, Token,
    parse_important,
};

use crate::declaration::Declaration;
use crate::error::{ErrorKind, ParseError, ParseWarning, Position};
use crate::selector::{
    AttributeMatcher, AttributeOperator, AttributeSelector, Combinator, CompoundSelector,
    PseudoClass, PseudoElement, Selector, SelectorComponent, SimpleSelector,
};
use crate::stylesheet::{AtRule, Rule, Stylesheet};

/// cssparser error that carries our own [`ErrorKind`] as its custom payload.
type CssError = CssParseError<ErrorKind>;

/// Builds a cssparser error out of one of our error kinds.
fn error(kind: ErrorKind) -> CssError {
    CssParseError::custom(kind)
}

// ---------------------------------------------------------------------------
// Stylesheet parsing
// ---------------------------------------------------------------------------

/// A top-level stylesheet item.
enum Item {
    /// A style rule.
    Rule(Rule),
    /// An `@` rule that was recorded and skipped.
    AtRule(AtRule),
}

/// Parses stylesheet source text into a [`Stylesheet`].
pub(crate) fn parse_stylesheet(css: &str) -> Result<Stylesheet, ParseError> {
    let mut input = Parser::new(css);
    let mut parser = TopLevelParser {
        warnings: Vec::new(),
    };
    let mut items = Vec::new();
    let mut failures: Vec<(ErrorKind, Position, &str)> = Vec::new();

    for result in StyleSheetParser::new(&mut input, &mut parser) {
        match result {
            Ok(item) => items.push(item),
            Err((css_error, source, location)) => failures.push((
                ErrorKind::from_css_error(&css_error),
                Position::from_source_location(location),
                source,
            )),
        }
    }
    let mut warnings = parser.warnings;

    let mut error = None;
    for (kind, position, source) in failures {
        if kind == ErrorKind::TooDeeplyNested {
            // The only genuinely fatal condition: we cannot trust anything else
            // in the file either, so stop and report it.
            error = Some(ParseError::at(kind, position));
        } else {
            warnings.push(ParseWarning::new(kind, position, source));
        }
    }
    if let Some(error) = error {
        return Err(error);
    }

    let mut rules = Vec::new();
    let mut at_rules = Vec::new();
    for item in items {
        match item {
            Item::Rule(rule) => rules.push(rule),
            Item::AtRule(at_rule) => at_rules.push(at_rule),
        }
    }

    warnings.sort_by_key(|warning| warning.position);
    Ok(Stylesheet::from_parts(rules, at_rules, warnings))
}

/// Parses the top level of a stylesheet: style rules and `@` rules.
struct TopLevelParser {
    warnings: Vec<ParseWarning>,
}

impl<'i> QualifiedRuleParser<'i> for TopLevelParser {
    type Prelude = Vec<Selector>;
    type QualifiedRule = Item;
    type Error = ErrorKind;

    fn parse_prelude(&mut self, input: &mut Parser<'i>) -> Result<Self::Prelude, CssError> {
        parse_selector_list(input)
    }

    fn parse_block(
        &mut self,
        selectors: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<Self::QualifiedRule, CssError> {
        let mut declarations = Vec::new();
        for result in RuleBodyParser::new(input, &mut DeclarationBlockParser) {
            match result {
                Ok(declaration) => declarations.push(declaration),
                Err((css_error, source, location)) => {
                    // Inside a declaration block every failure is a
                    // declaration failure, except the nesting limit, which is
                    // fatal wherever it happens.
                    let mut kind = ErrorKind::from_css_error(&css_error);
                    if kind != ErrorKind::TooDeeplyNested {
                        kind = ErrorKind::InvalidDeclaration;
                    }
                    self.warnings.push(ParseWarning::new(
                        kind,
                        Position::from_source_location(location),
                        source,
                    ));
                }
            }
        }
        Ok(Item::Rule(Rule {
            selectors,
            declarations,
        }))
    }
}

impl<'i> AtRuleParser<'i> for TopLevelParser {
    /// The lowercased at-rule name and its raw prelude text.
    type Prelude = (String, String);
    type AtRule = Item;
    type Error = ErrorKind;

    fn parse_prelude(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i>,
    ) -> Result<Self::Prelude, CssError> {
        let prelude = parse_raw_argument(input)?;
        Ok((name.to_ascii_lowercase(), prelude))
    }

    fn rule_without_block(
        &mut self,
        (name, prelude): Self::Prelude,
        _start: &ParserState,
    ) -> Result<Self::AtRule, ()> {
        Ok(Item::AtRule(AtRule { name, prelude }))
    }

    fn parse_block(
        &mut self,
        (name, prelude): Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<Self::AtRule, CssError> {
        // Consume the block so that parsing can resume after it. Its content is
        // deliberately not interpreted; see the crate-level limitations.
        while input.next().is_ok() {}
        Ok(Item::AtRule(AtRule { name, prelude }))
    }
}

/// Parses a declaration block: `color: red; margin: 0`.
struct DeclarationBlockParser;

impl<'i> DeclarationParser<'i> for DeclarationBlockParser {
    type Declaration = Declaration;
    type Error = ErrorKind;

    fn parse_value(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i>,
        _declaration_start: &ParserState,
    ) -> Result<Self::Declaration, CssError> {
        // The value runs to the `!` of `!important` or to the end of the
        // declaration, whichever comes first. `value_end` is dragged along after
        // every significant token so that trailing whitespace and comments do
        // not end up in the value.
        let value_start = input.position();
        let mut value_end = value_start;
        input.parse_until_before(cssparser::Delimiter::Bang, |input| {
            while skip_value_item(input)? {
                value_end = input.position();
            }
            Ok(())
        })?;
        let value = input.slice(value_start..value_end).trim();

        let mut important = false;
        if !input.is_exhausted() {
            // Something interrupted the value. The only legal reason is
            // `!important`; anything else makes the declaration invalid.
            input
                .try_parse(parse_important)
                .map_err(|_| error(ErrorKind::InvalidDeclaration))?;
            if !input.is_exhausted() {
                return Err(error(ErrorKind::InvalidDeclaration));
            }
            important = true;
        }

        if value.is_empty() {
            return Err(error(ErrorKind::InvalidDeclaration));
        }

        Ok(Declaration {
            name: Declaration::normalize_name(&name),
            value: value.to_owned(),
            important,
        })
    }
}

impl<'i> QualifiedRuleParser<'i> for DeclarationBlockParser {
    type Prelude = ();
    type QualifiedRule = Declaration;
    type Error = ErrorKind;
}

impl<'i> AtRuleParser<'i> for DeclarationBlockParser {
    type Prelude = ();
    type AtRule = Declaration;
    type Error = ErrorKind;
}

impl<'i> RuleBodyItemParser<'i, Declaration, ErrorKind> for DeclarationBlockParser {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        // CSS Nesting is not supported yet, so a nested style rule is treated
        // as an invalid declaration and skipped.
        false
    }
}

// ---------------------------------------------------------------------------
// Selector parsing
// ---------------------------------------------------------------------------

/// Parses a full selector list from `css`, requiring all of it to be valid.
pub(crate) fn parse_selector_list_str(css: &str) -> Result<Vec<Selector>, ParseError> {
    let mut input = Parser::new(css);
    let result = input.parse_entirely(parse_selector_list);
    let position = Position::from_source_location(input.current_source_location());
    result.map_err(|css_error| ParseError::at(ErrorKind::from_css_error(&css_error), position))
}

/// Skips whitespace and comments, and reports whether any whitespace was seen.
///
/// Comments do not count as whitespace: CSS removes them from the token stream
/// without inserting a space, so `a/**/b` is *not* the same as `a b`.
fn skip_whitespace_and_comments(input: &mut Parser<'_>) -> bool {
    let mut saw_whitespace = false;
    loop {
        let state = input.state();
        match input.next_including_whitespace_and_comments() {
            Ok(&Token::WhiteSpace(..)) => saw_whitespace = true,
            Ok(&Token::Comment(..)) => {}
            _ => {
                input.reset(&state);
                break;
            }
        }
    }
    saw_whitespace
}

/// Runs `check` only when it applies with no whitespace in front of it.
///
/// CSS does not allow whitespace between a `.`, a namespace `|`, or a pseudo
/// colon and the name that follows it. Comments are not whitespace and may
/// appear there.
fn try_adjacent<T>(
    input: &mut Parser<'_>,
    check: impl FnOnce(&mut Parser<'_>) -> Result<T, cssparser::BasicParseError>,
) -> Option<T> {
    let state = input.state();
    if skip_whitespace_and_comments(input) {
        input.reset(&state);
        return None;
    }
    match input.try_parse(check) {
        Ok(value) => Some(value),
        Err(_) => {
            input.reset(&state);
            None
        }
    }
}

/// Consumes `delim` when it is not preceded by whitespace.
fn try_expect_adjacent(input: &mut Parser<'_>, delim: char) -> bool {
    try_adjacent(input, |input| input.expect_delim(delim)).is_some()
}

/// Consumes a `:` when it is not preceded by whitespace.
fn try_expect_adjacent_colon(input: &mut Parser<'_>) -> bool {
    try_adjacent(input, |input| input.expect_colon()).is_some()
}

/// Parses a comma-separated list of selectors.
fn parse_selector_list(input: &mut Parser<'_>) -> Result<Vec<Selector>, CssError> {
    let mut selectors = Vec::new();
    loop {
        let components = parse_complex_selector_components(input)?;
        selectors.push(Selector::new(None, components));
        skip_whitespace_and_comments(input);
        if input.try_parse(|input| input.expect_comma()).is_err() {
            break;
        }
    }
    Ok(selectors)
}

/// Parses a comma-separated list of *relative* selectors, as `:has()` takes.
fn parse_relative_selector_list(input: &mut Parser<'_>) -> Result<Vec<Selector>, CssError> {
    let mut selectors = Vec::new();
    loop {
        let leading_combinator = parse_leading_combinator(input);
        let components = parse_complex_selector_components(input)?;
        selectors.push(Selector::new(leading_combinator, components));
        skip_whitespace_and_comments(input);
        if input.try_parse(|input| input.expect_comma()).is_err() {
            break;
        }
    }
    Ok(selectors)
}

/// Parses the optional combinator in front of a relative selector.
fn parse_leading_combinator(input: &mut Parser<'_>) -> Option<Combinator> {
    skip_whitespace_and_comments(input);
    if input.try_parse(|input| input.expect_delim('>')).is_ok() {
        Some(Combinator::Child)
    } else if input.try_parse(|input| input.expect_delim('+')).is_ok() {
        Some(Combinator::NextSibling)
    } else if input.try_parse(|input| input.expect_delim('~')).is_ok() {
        Some(Combinator::LaterSibling)
    } else {
        None
    }
}

/// Parses compound selectors joined by combinators.
fn parse_complex_selector_components(
    input: &mut Parser<'_>,
) -> Result<Vec<SelectorComponent>, CssError> {
    let mut components = vec![SelectorComponent {
        combinator: None,
        compound: parse_compound_selector(input)?,
    }];

    loop {
        let state = input.state();
        let saw_whitespace = skip_whitespace_and_comments(input);
        let combinator = if input.try_parse(|input| input.expect_delim('>')).is_ok() {
            Combinator::Child
        } else if input.try_parse(|input| input.expect_delim('+')).is_ok() {
            Combinator::NextSibling
        } else if input.try_parse(|input| input.expect_delim('~')).is_ok() {
            Combinator::LaterSibling
        } else if saw_whitespace {
            Combinator::Descendant
        } else {
            break;
        };

        // A trailing combinator with no compound selector after it is invalid.
        // Rewinding leaves the offending token for the caller's
        // `expect_exhausted` check to reject.
        let Ok(compound) = parse_compound_selector(input) else {
            input.reset(&state);
            break;
        };
        components.push(SelectorComponent {
            combinator: Some(combinator),
            compound,
        });
    }

    Ok(components)
}

/// Parses one compound selector, e.g. `div.note[data-x]:hover`.
fn parse_compound_selector(input: &mut Parser<'_>) -> Result<CompoundSelector, CssError> {
    skip_whitespace_and_comments(input);
    let mut simples = Vec::new();
    parse_type_selector(input, &mut simples)?;

    loop {
        // Whitespace ends a compound selector; `next()` would silently skip it
        // and glue `a :hover` into one compound selector.
        let state = input.state();
        if skip_whitespace_and_comments(input) {
            input.reset(&state);
            break;
        }
        match input.next() {
            Ok(Token::IDHash(id)) => {
                simples.push(SimpleSelector::Id(id.as_ref().to_owned()));
            }
            Ok(&Token::Delim('.')) => {
                if skip_whitespace_and_comments(input) {
                    return Err(error(ErrorKind::InvalidSelector));
                }
                let class = input
                    .expect_ident_cloned()
                    .map_err(|_| error(ErrorKind::InvalidSelector))?;
                simples.push(SimpleSelector::Class(class.to_string()));
            }
            Ok(&Token::SquareBracketBlock) => {
                let attribute = input.parse_nested_block(parse_attribute_selector)?;
                simples.push(SimpleSelector::Attribute(attribute));
            }
            Ok(&Token::Colon) => simples.push(parse_pseudo(input)?),
            _ => {
                input.reset(&state);
                break;
            }
        }
    }

    if simples.is_empty() {
        return Err(error(ErrorKind::InvalidSelector));
    }
    Ok(CompoundSelector { simples })
}

/// Parses the optional type or universal selector at the start of a compound
/// selector, including namespace prefixes.
fn parse_type_selector(
    input: &mut Parser<'_>,
    simples: &mut Vec<SimpleSelector>,
) -> Result<(), CssError> {
    // `|name`: an element in no namespace.
    if try_expect_adjacent(input, '|') {
        let name = parse_type_name(input)?;
        simples.push(SimpleSelector::Type(format!("|{name}")));
        return Ok(());
    }

    // `*`, `*|name` and `*|*`: the universal selector, optionally namespaced.
    if try_expect_adjacent(input, '*') {
        if try_expect_adjacent(input, '|') {
            let name = parse_type_name(input)?;
            simples.push(SimpleSelector::Type(format!("*|{name}")));
        } else {
            simples.push(SimpleSelector::Universal);
        }
        return Ok(());
    }

    // `name` or `ns|name`.
    let Ok(name) = input.try_parse(|input| input.expect_ident_cloned()) else {
        return Ok(());
    };
    if try_expect_adjacent(input, '|') {
        let local = parse_type_name(input)?;
        simples.push(SimpleSelector::Type(format!("{name}|{local}")));
    } else {
        simples.push(SimpleSelector::Type(name.to_string()));
    }
    Ok(())
}

/// Parses the local name of a namespaced type selector: an identifier or `*`.
fn parse_type_name(input: &mut Parser<'_>) -> Result<String, CssError> {
    if try_expect_adjacent(input, '*') {
        return Ok("*".to_owned());
    }
    // A namespace separator must be followed by the local name immediately.
    if skip_whitespace_and_comments(input) {
        return Err(error(ErrorKind::InvalidSelector));
    }
    input
        .expect_ident_cloned()
        .map(|name| name.to_string())
        .map_err(|_| error(ErrorKind::InvalidSelector))
}

/// Parses the inside of `[ … ]`.
fn parse_attribute_selector(input: &mut Parser<'_>) -> Result<AttributeSelector, CssError> {
    skip_whitespace_and_comments(input);
    let name = parse_attribute_name(input)?;
    skip_whitespace_and_comments(input);

    let state = input.state();
    let operator = match input.next() {
        Ok(&Token::Delim('=')) => Some(AttributeOperator::Equals),
        Ok(&Token::IncludeMatch) => Some(AttributeOperator::Includes),
        Ok(&Token::DashMatch) => Some(AttributeOperator::DashMatch),
        Ok(&Token::PrefixMatch) => Some(AttributeOperator::Prefix),
        Ok(&Token::SuffixMatch) => Some(AttributeOperator::Suffix),
        Ok(&Token::SubstringMatch) => Some(AttributeOperator::Substring),
        _ => {
            input.reset(&state);
            None
        }
    };

    let Some(operator) = operator else {
        return Ok(AttributeSelector {
            name,
            matcher: None,
            case_insensitive: false,
        });
    };

    skip_whitespace_and_comments(input);
    let value = match input.next() {
        Ok(Token::Ident(value)) | Ok(Token::QuotedString(value)) => value.as_ref().to_owned(),
        _ => return Err(error(ErrorKind::InvalidSelector)),
    };
    skip_whitespace_and_comments(input);

    // An optional `i` / `s` flag. `s` is the default and is not recorded.
    let mut case_insensitive = false;
    if let Ok(flag) = input.try_parse(|input| input.expect_ident_cloned()) {
        if flag.eq_ignore_ascii_case("i") {
            case_insensitive = true;
        } else if !flag.eq_ignore_ascii_case("s") {
            return Err(error(ErrorKind::InvalidSelector));
        }
    }

    Ok(AttributeSelector {
        name,
        matcher: Some(AttributeMatcher { operator, value }),
        case_insensitive,
    })
}

/// Parses an attribute name, keeping any namespace prefix.
fn parse_attribute_name(input: &mut Parser<'_>) -> Result<String, CssError> {
    let name = input
        .expect_ident_cloned()
        .map_err(|_| error(ErrorKind::InvalidSelector))?;
    if input.try_parse(|input| input.expect_delim('|')).is_ok() {
        let local = input
            .expect_ident_cloned()
            .map_err(|_| error(ErrorKind::InvalidSelector))?;
        return Ok(format!("{name}|{local}"));
    }
    Ok(name.to_string())
}

/// Parses a pseudo-class or pseudo-element; the leading `:` is already consumed.
fn parse_pseudo(input: &mut Parser<'_>) -> Result<SimpleSelector, CssError> {
    // A second, adjacent colon makes this a pseudo-element.
    let is_pseudo_element = try_expect_adjacent_colon(input);
    // No whitespace may separate the colons from the name.
    if skip_whitespace_and_comments(input) {
        return Err(error(ErrorKind::InvalidSelector));
    }

    let state = input.state();
    match input.next() {
        Ok(Token::Ident(name)) => {
            let name = name.as_ref().to_ascii_lowercase();
            if is_pseudo_element || is_legacy_pseudo_element(&name) {
                Ok(SimpleSelector::PseudoElement(PseudoElement {
                    name,
                    argument: None,
                }))
            } else {
                Ok(SimpleSelector::PseudoClass(PseudoClass::Other {
                    name,
                    argument: None,
                }))
            }
        }
        Ok(Token::Function(name)) => {
            let name = name.as_ref().to_ascii_lowercase();
            if is_pseudo_element {
                let argument = input.parse_nested_block(parse_raw_argument)?;
                return Ok(SimpleSelector::PseudoElement(PseudoElement {
                    name,
                    argument: Some(argument),
                }));
            }
            let pseudo_class = match name.as_str() {
                "is" => PseudoClass::Is(input.parse_nested_block(parse_selector_list)?),
                "where" => PseudoClass::Where(input.parse_nested_block(parse_selector_list)?),
                "not" => PseudoClass::Not(input.parse_nested_block(parse_selector_list)?),
                "has" => PseudoClass::Has(input.parse_nested_block(parse_relative_selector_list)?),
                _ => PseudoClass::Other {
                    name: name.clone(),
                    argument: Some(input.parse_nested_block(parse_raw_argument)?),
                },
            };
            Ok(SimpleSelector::PseudoClass(pseudo_class))
        }
        _ => {
            input.reset(&state);
            Err(error(ErrorKind::InvalidSelector))
        }
    }
}

/// The pseudo-elements CSS still accepts with a single colon.
///
/// They count as pseudo-elements for specificity and are normalised to a
/// [`PseudoElement`] so that nothing downstream has to know about them.
fn is_legacy_pseudo_element(name: &str) -> bool {
    matches!(name, "before" | "after" | "first-line" | "first-letter")
}

/// Advances past one value token, consuming a whole block when the token opens
/// one.
///
/// Returns `false` once the delimited input is exhausted. Block starts are
/// consumed eagerly (instead of being skipped lazily by the next `next()` call)
/// so that [`Parser::position`] always points at the real end of the value.
fn skip_value_item(input: &mut Parser<'_>) -> Result<bool, CssError> {
    let state = input.state();
    match input.next() {
        Ok(&Token::Function(_))
        | Ok(&Token::ParenthesisBlock)
        | Ok(&Token::SquareBracketBlock)
        | Ok(&Token::CurlyBracketBlock) => {
            input.parse_nested_block(|input| {
                while input.next().is_ok() {}
                Ok(())
            })?;
            Ok(true)
        }
        Ok(_) => Ok(true),
        Err(_) => {
            input.reset(&state);
            Ok(false)
        }
    }
}

/// Collects the raw text of the current delimitation, trimmed.
///
/// Used for arguments this crate does not model structurally, such as
/// `:nth-child(2n+1)` or an `@` rule prelude.
fn parse_raw_argument(input: &mut Parser<'_>) -> Result<String, CssError> {
    let start = input.position();
    while input.next().is_ok() {}
    Ok(input.slice_from(start).trim().to_owned())
}
