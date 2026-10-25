//! The declaration data model.
//!
//! A declaration is kept as an *unparsed* `name: value` pair. Validating and
//! interpreting values is the job of the computed-value layer, which does not
//! exist yet; keeping the raw text here means the syntax layer never has to
//! understand a property's grammar.

use std::fmt;

/// The name of a CSS custom property, i.e. a name starting with `--`.
const CUSTOM_PROPERTY_PREFIX: &str = "--";

/// A single `name: value` declaration inside a style rule.
///
/// ```
/// use jkva_style::Stylesheet;
///
/// let sheet = Stylesheet::parse("a { Color: Red !important; }").unwrap();
/// let declaration = &sheet.rules()[0].declarations[0];
/// assert_eq!(declaration.name, "color");
/// assert_eq!(declaration.value, "Red");
/// assert!(declaration.important);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Declaration {
    /// The property name, lowercased.
    ///
    /// CSS property names are ASCII case-insensitive, so `Color` is stored as
    /// `color`. Custom properties are the exception: their names are
    /// case-sensitive, so `--MyVar` keeps its case.
    pub name: String,
    /// The raw text of the value, with surrounding whitespace and comments
    /// removed.
    ///
    /// Whitespace *inside* the value is preserved exactly as written, so
    /// `margin: 0  auto` stores `"0  auto"`, and a comment in the middle of a
    /// value stays in the string. Values are not normalised or validated yet.
    pub value: String,
    /// Whether the declaration ended with `!important`.
    ///
    /// The cascade does not run in this crate yet, so nothing here acts on the
    /// flag; it is recorded so that the cascade can use it without re-parsing.
    pub important: bool,
}

impl Declaration {
    /// Creates a declaration from already-normalised parts.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>, important: bool) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            important,
        }
    }

    /// Returns `true` when this is a custom property declaration such as
    /// `--brand-color: red`.
    #[must_use]
    pub fn is_custom_property(&self) -> bool {
        self.name.starts_with(CUSTOM_PROPERTY_PREFIX)
    }

    /// Normalises a property name the way CSS requires.
    ///
    /// ASCII letters are lowercased unless the name starts with `--`, because
    /// custom property names are case-sensitive.
    #[must_use]
    pub fn normalize_name(name: &str) -> String {
        if name.starts_with(CUSTOM_PROPERTY_PREFIX) {
            name.to_owned()
        } else {
            name.to_ascii_lowercase()
        }
    }
}

impl fmt::Display for Declaration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.value)?;
        if self.important {
            f.write_str(" !important")?;
        }
        f.write_str(";")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_property_names_are_lowercased() {
        assert_eq!(
            Declaration::normalize_name("BackgroundColor"),
            "backgroundcolor"
        );
        assert_eq!(Declaration::normalize_name("color"), "color");
    }

    #[test]
    fn custom_property_names_keep_their_case() {
        assert_eq!(Declaration::normalize_name("--MyVar"), "--MyVar");
    }

    #[test]
    fn custom_properties_are_recognised() {
        assert!(Declaration::new("--x", "1px", false).is_custom_property());
        assert!(!Declaration::new("width", "1px", false).is_custom_property());
    }

    #[test]
    fn display_round_trips_the_important_flag() {
        assert_eq!(
            Declaration::new("color", "red", false).to_string(),
            "color: red;"
        );
        assert_eq!(
            Declaration::new("color", "red", true).to_string(),
            "color: red !important;"
        );
    }
}
