//! The selector data model and its specificity.
//!
//! Selectors are stored structurally rather than as text, because later stages
//! need to walk them: matching needs the individual simple selectors, the
//! cascade needs the specificity, and the invalidation layer needs the
//! rightmost compound selector to build a bucketed index.
//!
//! # What is modelled
//!
//! * Type selectors (`div`), including namespace prefixes kept verbatim
//!   (`svg|rect`), and the universal selector (`*`, `ns|*`, `*|*`).
//! * ID selectors (`#main`), class selectors (`.note`).
//! * Attribute selectors with no matcher (`[disabled]`) and all six matchers
//!   (`=`, `~=`, `|=`, `^=`, `$=`, `*=`) plus the `i` case-insensitivity flag.
//! * Pseudo-classes, including the selector-list pseudo-classes `:is()`,
//!   `:where()`, `:not()` and `:has()`, whose arguments are stored as nested
//!   selector lists.
//! * Pseudo-elements (`::before`, `::part(button)`) and the four legacy
//!   single-colon pseudo-elements, which are normalised to pseudo-elements
//!   because that is what their specificity and matching behaviour are.
//! * All four combinators: descendant, `>`, `+` and `~`.
//! * Relative selectors, i.e. a selector with a leading combinator, which only
//!   occur inside `:has()`.
//!
//! # What is *not* modelled yet
//!
//! * Namespace prefixes are preserved as written but never resolved; there is no
//!   `@namespace` handling, so matching must compare the prefix literally.
//! * The `of S` argument of `:nth-child()` and `:nth-last-child()` is not
//!   parsed; the whole argument is kept as opaque text and the specificity of
//!   `S` is therefore not counted. See [`PseudoClass`].
//! * The 2025 state-toggles (two-argument `:is()`, `:not()` with `&`) and the
//!   nesting selector `&` are not supported as arguments.
//! * Selector *matching* is not implemented here at all.

use std::fmt;

use crate::error::ParseError;
use crate::specificity::Specificity;

/// The combinator that links a compound selector to the one on its left.
///
/// The [`fmt::Display`] form is the CSS token that represents the combinator:
/// a single space for [`Combinator::Descendant`], otherwise the punctuation
/// character. Callers that build output by hand should add the surrounding
/// spaces themselves; [`Selector`]'s `Display` already does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Combinator {
    /// Whitespace: the element on the right is a descendant of the one on the
    /// left. Serialises as a single space.
    Descendant,
    /// `>`: the element on the right is a direct child.
    Child,
    /// `+`: the element on the right is the immediately following sibling.
    NextSibling,
    /// `~`: the element on the right is any following sibling.
    LaterSibling,
}

impl fmt::Display for Combinator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Descendant => " ",
            Self::Child => ">",
            Self::NextSibling => "+",
            Self::LaterSibling => "~",
        })
    }
}

/// One compound selector plus the combinator that links it to the compound
/// selector on its left.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SelectorComponent {
    /// The combinator linking this component to the previous one.
    ///
    /// This is `None` for the leftmost component of a selector, and `Some` for
    /// every other component.
    pub combinator: Option<Combinator>,
    /// The compound selector itself, e.g. `div.note[data-kind="x"]:hover`.
    pub compound: CompoundSelector,
}

/// A single selector, such as `main > article .title:hover`.
///
/// A selector is a list of compound selectors joined by combinators. Use
/// [`Selector::components`] to walk it and [`Selector::specificity`] to get its
/// specificity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Selector {
    leading_combinator: Option<Combinator>,
    components: Vec<SelectorComponent>,
}

impl Selector {
    /// Builds a selector from its leading combinator and components.
    ///
    /// `leading_combinator` is expected to be `Some` only for relative
    /// selectors, which appear exclusively inside `:has()`.
    #[must_use]
    pub fn new(leading_combinator: Option<Combinator>, components: Vec<SelectorComponent>) -> Self {
        Self {
            leading_combinator,
            components,
        }
    }

    /// The combinator in front of a relative selector, if it had one.
    ///
    /// A relative selector with no explicit combinator (`:has(img)`) is a
    /// descendant selector and reports `None`; `:has(> img)` reports
    /// `Some(Combinator::Child)`.
    #[must_use]
    pub const fn leading_combinator(&self) -> Option<Combinator> {
        self.leading_combinator
    }

    /// The compound selectors of this selector, left to right.
    ///
    /// The slice is never empty for a selector produced by the parser.
    #[must_use]
    pub fn components(&self) -> &[SelectorComponent] {
        &self.components
    }

    /// The specificity of this selector.
    #[must_use]
    pub fn specificity(&self) -> Specificity {
        let mut specificity = Specificity::ZERO;
        for component in &self.components {
            specificity += component.compound.specificity();
        }
        specificity
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(combinator) = self.leading_combinator {
            match combinator {
                Combinator::Descendant => {}
                other => write!(f, "{other} ")?,
            }
        }
        for (index, component) in self.components.iter().enumerate() {
            if index > 0 {
                match component.combinator {
                    Some(Combinator::Descendant) | None => f.write_str(" ")?,
                    Some(other) => write!(f, " {other} ")?,
                }
            }
            fmt::Display::fmt(&component.compound, f)?;
        }
        Ok(())
    }
}

/// A compound selector: simple selectors that all apply to the same element.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CompoundSelector {
    /// The simple selectors, in source order.
    ///
    /// The parser never produces an empty sequence, and always puts a type or
    /// universal selector first when one is present, because that is where CSS
    /// syntax requires it.
    pub simples: Vec<SimpleSelector>,
}

impl CompoundSelector {
    /// Creates a compound selector from its simple selectors.
    #[must_use]
    pub fn new(simples: Vec<SimpleSelector>) -> Self {
        Self { simples }
    }

    /// Number of simple selectors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.simples.len()
    }

    /// Returns `true` when there are no simple selectors.
    ///
    /// Only possible when a [`CompoundSelector`] was built by hand; the parser
    /// rejects empty compound selectors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.simples.is_empty()
    }

    /// The specificity of this compound selector.
    #[must_use]
    pub fn specificity(&self) -> Specificity {
        let mut specificity = Specificity::ZERO;
        for simple in &self.simples {
            specificity += simple.specificity();
        }
        specificity
    }
}

impl fmt::Display for CompoundSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for simple in &self.simples {
            fmt::Display::fmt(simple, f)?;
        }
        Ok(())
    }
}

/// A single simple selector.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SimpleSelector {
    /// The universal selector, `*`.
    ///
    /// Namespaced universals such as `ns|*` are stored as
    /// [`SimpleSelector::Type`] because the prefix matters for matching.
    Universal,
    /// A type selector: `div`, or `svg|rect` with a namespace prefix.
    ///
    /// The name is kept exactly as written. Type selectors are
    /// case-insensitive in HTML documents and case-sensitive elsewhere; that
    /// decision belongs to the matching layer, which knows the document type.
    Type(String),
    /// An ID selector, e.g. `#main` for `#main`.
    Id(String),
    /// A class selector, e.g. `note` for `.note`.
    Class(String),
    /// An attribute selector, e.g. `[type="text" i]`.
    Attribute(AttributeSelector),
    /// A pseudo-class, e.g. `:hover`, `:is(.a, .b)`.
    PseudoClass(PseudoClass),
    /// A pseudo-element, e.g. `::before`.
    PseudoElement(PseudoElement),
}

impl SimpleSelector {
    /// The specificity contributed by this simple selector.
    #[must_use]
    pub fn specificity(&self) -> Specificity {
        match self {
            Self::Universal => Specificity::ZERO,
            Self::Type(_) => Specificity::new(0, 0, 1),
            Self::Id(_) => Specificity::new(1, 0, 0),
            Self::Class(_) | Self::Attribute(_) => Specificity::new(0, 1, 0),
            Self::PseudoClass(pseudo) => pseudo.specificity(),
            Self::PseudoElement(_) => Specificity::new(0, 0, 1),
        }
    }
}

impl fmt::Display for SimpleSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Universal => f.write_str("*"),
            Self::Type(name) => f.write_str(name),
            Self::Id(id) => write!(f, "#{}", write_identifier(id)),
            Self::Class(class) => write!(f, ".{}", write_identifier(class)),
            Self::Attribute(attribute) => fmt::Display::fmt(attribute, f),
            Self::PseudoClass(pseudo) => fmt::Display::fmt(pseudo, f),
            Self::PseudoElement(pseudo) => fmt::Display::fmt(pseudo, f),
        }
    }
}

/// An attribute selector, e.g. `[disabled]` or `[type^="img" i]`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AttributeSelector {
    /// The attribute name exactly as written, including any namespace prefix.
    pub name: String,
    /// The matcher, or `None` for the presence test `[attr]`.
    pub matcher: Option<AttributeMatcher>,
    /// `true` when the `i` (ASCII case-insensitive) flag was given.
    ///
    /// The explicit `s` flag is the default and is not recorded; it is
    /// indistinguishable from having no flag at all.
    pub case_insensitive: bool,
}

impl fmt::Display for AttributeSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}", self.name)?;
        if let Some(matcher) = &self.matcher {
            write!(
                f,
                "{}{}",
                matcher.operator,
                write_css_string(&matcher.value)
            )?;
            if self.case_insensitive {
                f.write_str(" i")?;
            }
        }
        f.write_str("]")
    }
}

/// The comparison part of an attribute selector.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AttributeMatcher {
    /// How the attribute value is compared.
    pub operator: AttributeOperator,
    /// The value to compare against.
    ///
    /// The value is always serialised back as a double-quoted string; the
    /// distinction between `[a=b]` and `[a="b"]` is not recorded because the two
    /// are equivalent in CSS.
    pub value: String,
}

/// The operator of an attribute matcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttributeOperator {
    /// `=`: the value is exactly equal.
    Equals,
    /// `~=`: the value appears in a whitespace-separated list.
    Includes,
    /// `|=`: the value is equal to, or starts with the value followed by `-`.
    DashMatch,
    /// `^=`: the value starts with the given prefix.
    Prefix,
    /// `$=`: the value ends with the given suffix.
    Suffix,
    /// `*=`: the value contains the given substring.
    Substring,
}

impl AttributeOperator {
    /// The CSS token for this operator, without surrounding whitespace.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Equals => "=",
            Self::Includes => "~=",
            Self::DashMatch => "|=",
            Self::Prefix => "^=",
            Self::Suffix => "$=",
            Self::Substring => "*=",
        }
    }
}

impl fmt::Display for AttributeOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A pseudo-class.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PseudoClass {
    /// `:is(...)`: matches if any argument matches. Contributes the specificity
    /// of its most specific argument.
    Is(Vec<Selector>),
    /// `:where(...)`: matches like `:is()` but always contributes zero
    /// specificity.
    Where(Vec<Selector>),
    /// `:not(...)`: matches if no argument matches. Contributes the specificity
    /// of its most specific argument.
    Not(Vec<Selector>),
    /// `:has(...)`: matches if a relative selector matches. Contributes the
    /// specificity of its most specific argument.
    Has(Vec<Selector>),
    /// Any other pseudo-class.
    ///
    /// `name` is ASCII-lowercased (pseudo-class names are case-insensitive) and
    /// `argument` holds the raw argument text for functional pseudo-classes,
    /// e.g. `Some("2n+1")` for `:nth-child(2n+1)`.
    ///
    /// These count as one class-level selector. Note that the `of S` part of
    /// `:nth-child(An+B of S)` and `:nth-last-child(An+B of S)` is not parsed,
    /// so the specificity of `S` is not counted; this understates the
    /// specificity of those selectors.
    Other {
        /// The lowercased pseudo-class name, without the leading colon.
        name: String,
        /// Raw argument text for a functional pseudo-class.
        argument: Option<String>,
    },
}

impl PseudoClass {
    /// The specificity contributed by this pseudo-class.
    #[must_use]
    pub fn specificity(&self) -> Specificity {
        match self {
            Self::Is(selectors) | Self::Not(selectors) | Self::Has(selectors) => {
                most_specific(selectors)
            }
            Self::Where(_) => Specificity::ZERO,
            Self::Other { .. } => Specificity::new(0, 1, 0),
        }
    }

    /// The lowercased name of the pseudo-class, without the leading colon.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Is(_) => "is",
            Self::Where(_) => "where",
            Self::Not(_) => "not",
            Self::Has(_) => "has",
            Self::Other { name, .. } => name,
        }
    }
}

impl fmt::Display for PseudoClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Is(selectors) => write!(f, ":is({})", write_selector_list(selectors)),
            Self::Where(selectors) => write!(f, ":where({})", write_selector_list(selectors)),
            Self::Not(selectors) => write!(f, ":not({})", write_selector_list(selectors)),
            Self::Has(selectors) => write!(f, ":has({})", write_selector_list(selectors)),
            Self::Other { name, argument } => match argument {
                Some(argument) => write!(f, ":{name}({argument})"),
                None => write!(f, ":{name}"),
            },
        }
    }
}

/// A pseudo-element.
///
/// The four legacy single-colon pseudo-elements (`:before`, `:after`,
/// `:first-line`, `:first-letter`) are normalised to pseudo-elements by the
/// parser and serialise with a double colon.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PseudoElement {
    /// The lowercased pseudo-element name, without the leading `::`.
    pub name: String,
    /// Raw argument text for functional pseudo-elements such as
    /// `::part(button)` or `::slotted(span)`.
    pub argument: Option<String>,
}

impl fmt::Display for PseudoElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.argument {
            Some(argument) => write!(f, "::{}({argument})", self.name),
            None => write!(f, "::{}", self.name),
        }
    }
}

/// Parses a selector list, as used by `Element.querySelector()` or a style
/// rule's prelude.
///
/// The whole input must be consumed: trailing junk such as `div,` or `div *` in
/// the wrong place is an error, and so is any invalid selector in the list,
/// because CSS invalidates the entire list rather than the single item.
///
/// ```
/// use jkva_style::parse_selector_list;
///
/// let selectors = parse_selector_list("main > .title, #hero").unwrap();
/// assert_eq!(selectors.len(), 2);
/// assert_eq!(selectors[0].to_string(), "main > .title");
/// assert!(parse_selector_list("div >").is_err());
/// ```
pub fn parse_selector_list(css: &str) -> Result<Vec<Selector>, ParseError> {
    crate::parser::parse_selector_list_str(css)
}

/// Returns the highest specificity in `selectors`, or zero when it is empty.
fn most_specific(selectors: &[Selector]) -> Specificity {
    selectors
        .iter()
        .map(Selector::specificity)
        .max()
        .unwrap_or(Specificity::ZERO)
}

/// Writes a selector list as `a, b, c`.
fn write_selector_list(selectors: &[Selector]) -> String {
    selectors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Writes an identifier, escaping the characters that would change how it
/// parses.
///
/// Only the escape-hatch characters are handled; full CSSOM identifier escaping
/// (`\31 23` for `#123`) is not implemented because the parser never produces
/// names that need it — it only accepts valid identifiers.
fn write_identifier(identifier: &str) -> String {
    let mut out = String::with_capacity(identifier.len());
    for character in identifier.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            other => out.push(other),
        }
    }
    out
}

/// Writes a double-quoted CSS string.
fn write_css_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' | '\\' => {
                out.push('\\');
                out.push(character);
            }
            '\n' => out.push_str("\\a "),
            '\r' => out.push_str("\\d "),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combinators_serialise_as_their_css_tokens() {
        assert_eq!(Combinator::Descendant.to_string(), " ");
        assert_eq!(Combinator::Child.to_string(), ">");
        assert_eq!(Combinator::NextSibling.to_string(), "+");
        assert_eq!(Combinator::LaterSibling.to_string(), "~");
    }

    #[test]
    fn attribute_values_are_always_double_quoted() {
        let attribute = AttributeSelector {
            name: "type".to_owned(),
            matcher: Some(AttributeMatcher {
                operator: AttributeOperator::Equals,
                value: "text".to_owned(),
            }),
            case_insensitive: false,
        };
        assert_eq!(attribute.to_string(), "[type=\"text\"]");
    }

    #[test]
    fn attribute_strings_escape_quotes_and_backslashes() {
        let attribute = AttributeSelector {
            name: "data-x".to_owned(),
            matcher: Some(AttributeMatcher {
                operator: AttributeOperator::Substring,
                value: "a\"b\\c".to_owned(),
            }),
            case_insensitive: true,
        };
        assert_eq!(attribute.to_string(), "[data-x*=\"a\\\"b\\\\c\" i]");
    }

    #[test]
    fn pseudo_classes_serialise_their_argument() {
        let pseudo = PseudoClass::Other {
            name: "nth-child".to_owned(),
            argument: Some("2n+1".to_owned()),
        };
        assert_eq!(pseudo.to_string(), ":nth-child(2n+1)");
        let pseudo = PseudoClass::Other {
            name: "hover".to_owned(),
            argument: None,
        };
        assert_eq!(pseudo.to_string(), ":hover");
    }

    #[test]
    fn empty_pseudo_class_arguments_have_zero_specificity() {
        assert!(PseudoClass::Where(Vec::new()).specificity().is_zero());
        assert!(PseudoClass::Is(Vec::new()).specificity().is_zero());
    }

    #[test]
    fn specificity_of_a_hand_built_selector_sums_its_components() {
        let selector = Selector::new(
            None,
            vec![
                SelectorComponent {
                    combinator: None,
                    compound: CompoundSelector::new(vec![SimpleSelector::Type("div".into())]),
                },
                SelectorComponent {
                    combinator: Some(Combinator::Child),
                    compound: CompoundSelector::new(vec![
                        SimpleSelector::Class("note".into()),
                        SimpleSelector::PseudoElement(PseudoElement {
                            name: "before".into(),
                            argument: None,
                        }),
                    ]),
                },
            ],
        );
        assert_eq!(selector.specificity(), Specificity::new(0, 1, 2));
        assert_eq!(selector.to_string(), "div > .note::before");
    }

    #[test]
    fn relative_selectors_serialise_their_leading_combinator() {
        let selector = Selector::new(
            Some(Combinator::Child),
            vec![SelectorComponent {
                combinator: None,
                compound: CompoundSelector::new(vec![SimpleSelector::Type("img".into())]),
            }],
        );
        assert_eq!(selector.to_string(), "> img");
        assert_eq!(selector.specificity(), Specificity::new(0, 0, 1));
    }
}
