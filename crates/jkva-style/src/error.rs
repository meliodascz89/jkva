//! Parse errors and parse warnings.
//!
//! CSS parsing is error tolerant: a malformed rule or declaration is dropped and
//! parsing continues with the next one. Only conditions that make the whole
//! stylesheet unusable are reported as an [`Error`] from
//! [`Stylesheet::parse`](crate::Stylesheet::parse); everything else is reported
//! as a [`ParseWarning`] and recorded on the resulting stylesheet.

use std::fmt;

use cssparser::{BasicParseErrorKind, ParseError as CssParseError, ParseErrorKind};

/// A position in a stylesheet.
///
/// Both fields are 1-based so that they can be shown to humans and compared with
/// what an editor reports. cssparser counts columns in UTF-16 code units, which
/// is what the CSS specifications do; a column therefore is not a byte offset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number, counted in UTF-16 code units.
    pub column: u32,
}

impl Position {
    /// Creates a position from a 1-based line and column.
    #[must_use]
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }

    /// Converts a cssparser source location into a [`Position`].
    ///
    /// cssparser numbers lines from zero; this method adds one so that the
    /// result is 1-based like the rest of this crate.
    #[must_use]
    pub(crate) const fn from_source_location(location: cssparser::SourceLocation) -> Self {
        Self {
            line: location.line + 1,
            column: location.column,
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Why a piece of CSS could not be parsed.
///
/// The same enum is used for hard errors ([`Error`]) and for recoverable
/// problems ([`ParseWarning`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The input ended in the middle of a rule, declaration or selector.
    UnexpectedEndOfInput,
    /// A token appeared where the grammar does not allow one.
    UnexpectedToken,
    /// A selector list could not be parsed, for example `div > > p`.
    InvalidSelector,
    /// A style rule could not be parsed.
    InvalidRule,
    /// The prelude of an `@` rule could not be parsed.
    InvalidAtRule,
    /// The body of an `@` rule could not be parsed.
    InvalidAtRuleBody,
    /// A single declaration could not be parsed and was dropped.
    ///
    /// This includes declarations with an empty value and declarations whose
    /// trailing `!` sequence is not `!important`.
    InvalidDeclaration,
    /// The input nests blocks deeper than the parser is willing to follow.
    ///
    /// This is the only condition that aborts parsing of a whole stylesheet.
    TooDeeplyNested,
}

impl ErrorKind {
    /// Returns a short, stable, human-readable description.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnexpectedEndOfInput => "unexpected end of input",
            Self::UnexpectedToken => "unexpected token",
            Self::InvalidSelector => "invalid selector",
            Self::InvalidRule => "invalid rule",
            Self::InvalidAtRule => "invalid at-rule",
            Self::InvalidAtRuleBody => "invalid at-rule body",
            Self::InvalidDeclaration => "invalid declaration",
            Self::TooDeeplyNested => "input nests blocks too deeply",
        }
    }

    /// Maps a cssparser built-in error kind onto the closest [`ErrorKind`].
    ///
    /// cssparser does not distinguish a bad `@` rule prelude from a bad style
    /// rule prelude, so the caller is expected to override the mapping where it
    /// knows more.
    #[must_use]
    pub const fn from_basic(kind: BasicParseErrorKind) -> Self {
        match kind {
            BasicParseErrorKind::UnexpectedToken => Self::UnexpectedToken,
            BasicParseErrorKind::EndOfInput => Self::UnexpectedEndOfInput,
            BasicParseErrorKind::AtRuleInvalid => Self::InvalidAtRule,
            BasicParseErrorKind::AtRuleBodyInvalid => Self::InvalidAtRuleBody,
            BasicParseErrorKind::QualifiedRuleInvalid => Self::InvalidRule,
            BasicParseErrorKind::TooManyNestedBlocks => Self::TooDeeplyNested,
        }
    }

    /// Classifies a cssparser error that may carry a custom [`ErrorKind`].
    #[must_use]
    pub(crate) fn from_css_error(error: &CssParseError<Self>) -> Self {
        match &error.kind {
            ParseErrorKind::Basic(basic) => Self::from_basic(basic.clone()),
            ParseErrorKind::Custom(kind) => *kind,
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A fatal stylesheet parse error.
///
/// `jkva-style` produces this only for input that cannot be interpreted at all
/// (see [`ErrorKind::TooDeeplyNested`]). Everything CSS defines as recoverable
/// is reported through [`ParseWarning`] instead, because dropping a rule and
/// carrying on is what browsers do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    kind: ErrorKind,
    position: Option<Position>,
}

impl ParseError {
    /// Creates an error without a source position.
    #[must_use]
    pub const fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            position: None,
        }
    }

    /// Creates an error with the position where it was detected.
    #[must_use]
    pub const fn at(kind: ErrorKind, position: Position) -> Self {
        Self {
            kind,
            position: Some(position),
        }
    }

    /// Returns what went wrong.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns where the problem was detected, when the input is known.
    #[must_use]
    pub const fn position(&self) -> Option<Position> {
        self.position
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.position {
            Some(position) => write!(f, "{position}: {}", self.kind),
            None => fmt::Display::fmt(&self.kind, f),
        }
    }
}

impl std::error::Error for ParseError {}

/// A recoverable problem found while parsing a stylesheet.
///
/// Warnings never stop parsing; they exist so that a caller (the engine, a
/// developer tool, a test) can see what was dropped instead of only noticing
/// that some rule failed to apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseWarning {
    /// What was wrong.
    pub kind: ErrorKind,
    /// Where the offending construct starts.
    pub position: Position,
    /// The source text of the offending construct, truncated for readability.
    ///
    /// The text is copied out of the stylesheet so that a warning remains valid
    /// after the stylesheet source is dropped.
    pub source: String,
}

impl ParseWarning {
    /// Longest source snippet kept on a warning, in bytes.
    ///
    /// The snippet is cut on a character boundary and gets an ellipsis appended.
    pub const MAX_SOURCE_LEN: usize = 120;

    /// Creates a warning, truncating `source` to [`Self::MAX_SOURCE_LEN`].
    #[must_use]
    pub fn new(kind: ErrorKind, position: Position, source: &str) -> Self {
        Self {
            kind,
            position,
            source: truncate(source.trim(), Self::MAX_SOURCE_LEN),
        }
    }
}

impl fmt::Display for ParseWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.position, self.kind, self.source)
    }
}

/// Shortens `text` to at most `max` bytes without splitting a UTF-8 character.
fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = text[..end].to_owned();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_short_text() {
        assert_eq!(truncate("color: red", 32), "color: red");
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        // "ě" is two bytes; cutting at an odd byte offset must walk back.
        let text = "aaaaaaaaaě";
        let cut = truncate(text, 10);
        assert_eq!(cut, "aaaaaaaaa…");
    }

    #[test]
    fn warning_truncates_source() {
        let source = "x".repeat(500);
        let warning = ParseWarning::new(ErrorKind::InvalidRule, Position::new(1, 1), &source);
        assert!(warning.source.len() <= ParseWarning::MAX_SOURCE_LEN + '…'.len_utf8());
        assert!(warning.source.ends_with('…'));
    }

    #[test]
    fn positions_format_as_line_and_column() {
        assert_eq!(Position::new(3, 12).to_string(), "3:12");
    }
}
