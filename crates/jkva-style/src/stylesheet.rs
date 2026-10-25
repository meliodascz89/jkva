//! The stylesheet, rule and at-rule data model.

use std::fmt;
use std::str::FromStr;

use crate::declaration::Declaration;
use crate::error::{ParseError, ParseWarning};
use crate::selector::Selector;
use crate::specificity::Specificity;

/// One style rule: a selector list plus its declaration block.
///
/// Each selector in [`Rule::selectors`] matches independently and carries its
/// own specificity. A rule is therefore *not* a unit the cascade can rank; it
/// has to be expanded into `(selector, specificity, declarations)` triples that
/// all share the rule's source order. [`Rule::selector_specificities`] exists to
/// make that expansion trivial.
///
/// ```
/// use jkva_style::Stylesheet;
///
/// let sheet = Stylesheet::parse("h1, .title { color: red }").unwrap();
/// let rule = &sheet.rules()[0];
/// assert_eq!(rule.selectors.len(), 2);
/// assert_eq!(rule.declarations[0].value, "red");
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rule {
    /// The comma-separated selectors of this rule, in source order.
    pub selectors: Vec<Selector>,
    /// The declarations of this rule's block, in source order.
    ///
    /// Declarations that failed to parse are dropped and reported as
    /// [`ParseWarning`]s on the stylesheet. Duplicate properties are kept: the
    /// cascade decides which one wins.
    pub declarations: Vec<Declaration>,
}

impl Rule {
    /// Iterates over each selector with its own specificity.
    pub fn selector_specificities(&self) -> impl Iterator<Item = (&Selector, Specificity)> {
        self.selectors
            .iter()
            .map(|selector| (selector, selector.specificity()))
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, selector) in self.selectors.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            fmt::Display::fmt(selector, f)?;
        }
        f.write_str(" {")?;
        for declaration in &self.declarations {
            write!(f, " {declaration}")?;
        }
        f.write_str(" }")
    }
}

/// An `@` rule that this crate records but does not understand yet.
///
/// Recording the name and prelude is deliberate: a browser engine needs to be
/// able to report *why* a stylesheet lost rules, and a later task can decide
/// per at-rule whether to apply it, ignore it, or hoist it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AtRule {
    /// The ASCII-lowercased at-rule name, without the leading `@`.
    pub name: String,
    /// The raw, trimmed prelude between the name and the block or semicolon.
    ///
    /// Empty for at-rules without a prelude, such as `@media { … }`.
    pub prelude: String,
}

impl fmt::Display for AtRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.prelude.is_empty() {
            write!(f, "@{}", self.name)
        } else {
            write!(f, "@{} {}", self.name, self.prelude)
        }
    }
}

/// A parsed CSS stylesheet: the style rules plus what had to be skipped.
///
/// The rules are in source order, which the cascade needs as its tie-breaker.
///
/// ```
/// use jkva_style::Stylesheet;
///
/// let sheet = Stylesheet::parse("div { color: red }").unwrap();
/// assert_eq!(sheet.rules().len(), 1);
/// assert!(!sheet.is_empty());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stylesheet {
    rules: Vec<Rule>,
    at_rules: Vec<AtRule>,
    warnings: Vec<ParseWarning>,
}

impl Stylesheet {
    /// Creates an empty stylesheet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Assembles a stylesheet from already parsed parts.
    ///
    /// Intended for the parser and for tests; the parts are not validated
    /// against each other.
    #[must_use]
    pub(crate) fn from_parts(
        rules: Vec<Rule>,
        at_rules: Vec<AtRule>,
        warnings: Vec<ParseWarning>,
    ) -> Self {
        Self {
            rules,
            at_rules,
            warnings,
        }
    }

    /// Parses stylesheet source text.
    ///
    /// Parsing is lenient in the same way browsers are lenient: an invalid
    /// selector invalidates its rule, an invalid declaration is dropped, and
    /// `@` rules are recorded and skipped. All of that is reported through
    /// [`Stylesheet::warnings`] and never aborts the parse.
    ///
    /// # Errors
    ///
    /// Returns an error only when the input cannot be interpreted at all, which
    /// in practice means it nests blocks deeper than the parser is willing to
    /// follow ([`ErrorKind::TooDeeplyNested`](crate::ErrorKind::TooDeeplyNested)).
    ///
    /// ```
    /// use jkva_style::Stylesheet;
    ///
    /// let sheet = Stylesheet::parse("div { color: red }").unwrap();
    /// assert_eq!(sheet.rules().len(), 1);
    /// ```
    pub fn parse(css: &str) -> Result<Self, ParseError> {
        crate::parser::parse_stylesheet(css)
    }

    /// Returns the source text of every rule, in source order.
    ///
    /// This is handy for debugging and for tests; the engine itself should walk
    /// [`Stylesheet::rules`] instead.
    #[must_use]
    pub fn to_css_string(&self) -> String {
        self.rules
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The style rules of this stylesheet, in source order.
    #[must_use]
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// The `@` rules that were found and skipped, in source order.
    #[must_use]
    pub fn at_rules(&self) -> &[AtRule] {
        &self.at_rules
    }

    /// Everything that was dropped or skipped while parsing, in source order.
    #[must_use]
    pub fn warnings(&self) -> &[ParseWarning] {
        &self.warnings
    }

    /// Number of style rules.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Returns `true` when no style rule was parsed.
    ///
    /// Note that a stylesheet can be empty and still have at-rules or warnings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Iterates over the style rules in source order.
    pub fn iter(&self) -> std::slice::Iter<'_, Rule> {
        self.rules.iter()
    }
}

impl FromStr for Stylesheet {
    type Err = ParseError;

    fn from_str(css: &str) -> Result<Self, Self::Err> {
        Self::parse(css)
    }
}

impl<'a> IntoIterator for &'a Stylesheet {
    type Item = &'a Rule;
    type IntoIter = std::slice::Iter<'a, Rule>;

    fn into_iter(self) -> Self::IntoIter {
        self.rules.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_round_trips_a_rule() {
        let sheet = Stylesheet::parse("div, .a { color: red !important; width: 1px }").unwrap();
        assert_eq!(
            sheet.rules()[0].to_string(),
            "div, .a { color: red !important; width: 1px; }"
        );
    }

    #[test]
    fn display_of_at_rules_is_reconstructible() {
        let at_rule = AtRule {
            name: "media".to_owned(),
            prelude: "(min-width: 10px)".to_owned(),
        };
        assert_eq!(at_rule.to_string(), "@media (min-width: 10px)");
        let bare = AtRule {
            name: "font-face".to_owned(),
            prelude: String::new(),
        };
        assert_eq!(bare.to_string(), "@font-face");
    }

    #[test]
    fn empty_stylesheet_is_empty() {
        let sheet = Stylesheet::parse("").unwrap();
        assert!(sheet.is_empty());
        assert_eq!(sheet.len(), 0);
        assert!(sheet.warnings().is_empty());
    }
}
