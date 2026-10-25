//! The computed style of one element.
//!
//! This is the first, deliberately small computed-value model: it covers the
//! properties M1 layout needs to get off the ground.
//!
//! # What "computed" means here
//!
//! The value of a property is the winner of the cascade (see
//! [`crate::cascade`]) after shorthand expansion, CSS-wide keywords
//! (`inherit`, `initial`, `unset`) and inheritance have been applied.
//!
//! Values are **not** absolution yet: `width: 50%` stays `"50%"`, `color: red`
//! stays `"red"`, `font-size: 2em` stays `"2em"` and `var(--x)` stays
//! `"var(--x)"`. Turning these into pixels and colour values needs a value
//! parser and a font context, neither of which exists. Display is the one
//! exception: it is parsed into [`DisplayType`] because layout branches on it.

use std::fmt;

/// The value of the `display` property.
///
/// Covers the single-keyword values of CSS Display 3 plus the legacy
/// `inline-block`, `inline-table`, `inline-flex` and `inline-grid` spellings.
/// Multi-keyword values (`display: inline flow-root`, `display: block flex`) are
/// not supported: they do not parse, so the declaration is dropped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DisplayType {
    /// `display: none` — no box at all.
    None,
    /// `display: contents` — the box disappears, the children stay.
    Contents,
    /// `display: block`.
    Block,
    /// `display: inline`. The CSS initial value.
    #[default]
    Inline,
    /// `display: inline-block`.
    InlineBlock,
    /// `display: flow-root`.
    FlowRoot,
    /// `display: inline flow-root` (`display: inline-block` in Display 4).
    InlineFlowRoot,
    /// `display: list-item`.
    ListItem,
    /// `display: inline list-item`.
    InlineListItem,
    /// `display: flex`.
    Flex,
    /// `display: inline-flex`.
    InlineFlex,
    /// `display: grid`.
    Grid,
    /// `display: inline-grid`.
    InlineGrid,
    /// `display: table`.
    Table,
    /// `display: inline-table`.
    InlineTable,
    /// `display: table-row-group`.
    TableRowGroup,
    /// `display: table-header-group`.
    TableHeaderGroup,
    /// `display: table-footer-group`.
    TableFooterGroup,
    /// `display: table-row`.
    TableRow,
    /// `display: table-cell`.
    TableCell,
    /// `display: table-column-group`.
    TableColumnGroup,
    /// `display: table-column`.
    TableColumn,
    /// `display: table-caption`.
    TableCaption,
}

impl DisplayType {
    /// Parses a `display` keyword, ignoring ASCII case.
    ///
    /// Returns `None` for anything that is not a single supported keyword, which
    /// includes the CSS-wide keywords (`inherit`, `initial`, `unset`, `revert`)
    /// and `var()`.
    #[must_use]
    pub fn from_keyword(value: &str) -> Option<Self> {
        let keyword = value.trim().to_ascii_lowercase();
        Some(match keyword.as_str() {
            "none" => Self::None,
            "contents" => Self::Contents,
            "block" => Self::Block,
            "inline" => Self::Inline,
            "inline-block" => Self::InlineBlock,
            "flow-root" => Self::FlowRoot,
            "inline-flow-root" => Self::InlineFlowRoot,
            "list-item" => Self::ListItem,
            "inline-list-item" => Self::InlineListItem,
            "flex" => Self::Flex,
            "inline-flex" => Self::InlineFlex,
            "grid" => Self::Grid,
            "inline-grid" => Self::InlineGrid,
            "table" => Self::Table,
            "inline-table" => Self::InlineTable,
            "table-row-group" => Self::TableRowGroup,
            "table-header-group" => Self::TableHeaderGroup,
            "table-footer-group" => Self::TableFooterGroup,
            "table-row" => Self::TableRow,
            "table-cell" => Self::TableCell,
            "table-column-group" => Self::TableColumnGroup,
            "table-column" => Self::TableColumn,
            "table-caption" => Self::TableCaption,
            _ => return None,
        })
    }

    /// The CSS keyword for this display type.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Contents => "contents",
            Self::Block => "block",
            Self::Inline => "inline",
            Self::InlineBlock => "inline-block",
            Self::FlowRoot => "flow-root",
            Self::InlineFlowRoot => "inline-flow-root",
            Self::ListItem => "list-item",
            Self::InlineListItem => "inline-list-item",
            Self::Flex => "flex",
            Self::InlineFlex => "inline-flex",
            Self::Grid => "grid",
            Self::InlineGrid => "inline-grid",
            Self::Table => "table",
            Self::InlineTable => "inline-table",
            Self::TableRowGroup => "table-row-group",
            Self::TableHeaderGroup => "table-header-group",
            Self::TableFooterGroup => "table-footer-group",
            Self::TableRow => "table-row",
            Self::TableCell => "table-cell",
            Self::TableColumnGroup => "table-column-group",
            Self::TableColumn => "table-column",
            Self::TableCaption => "table-caption",
        }
    }

    /// Returns `true` when the box is not generated (`display: none`).
    #[must_use]
    pub const fn is_none(self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns `true` for the `inline-*` display types.
    #[must_use]
    pub const fn is_inline(self) -> bool {
        matches!(
            self,
            Self::Inline
                | Self::InlineBlock
                | Self::InlineFlowRoot
                | Self::InlineListItem
                | Self::InlineFlex
                | Self::InlineGrid
                | Self::InlineTable
        )
    }
}

impl fmt::Display for DisplayType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The computed style of one element.
///
/// Only the properties listed here are tracked. Every other declaration in the
/// stylesheet is ignored, including the `background` and `font` shorthands.
///
/// Values are raw specified-value text except for [`ComputedStyle::display`];
/// see the module docs.
///
/// [`Default`] is implemented as [`ComputedStyle::initial`], not as "every field
/// empty", so a default-constructed style is a valid style for an element that
/// no rule matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputedStyle {
    /// The `display` property.
    pub display: DisplayType,
    /// The `color` property. Inherited.
    pub color: String,
    /// The `background-color` property. Not inherited.
    pub background_color: String,
    /// The `width` property. Not inherited.
    pub width: String,
    /// The `height` property. Not inherited.
    pub height: String,
    /// The `margin-top` property. Not inherited.
    pub margin_top: String,
    /// The `margin-right` property. Not inherited.
    pub margin_right: String,
    /// The `margin-bottom` property. Not inherited.
    pub margin_bottom: String,
    /// The `margin-left` property. Not inherited.
    pub margin_left: String,
    /// The `padding-top` property. Not inherited.
    pub padding_top: String,
    /// The `padding-right` property. Not inherited.
    pub padding_right: String,
    /// The `padding-bottom` property. Not inherited.
    pub padding_bottom: String,
    /// The `padding-left` property. Not inherited.
    pub padding_left: String,
    /// The `border-top-width` property. Not inherited.
    pub border_top_width: String,
    /// The `border-right-width` property. Not inherited.
    pub border_right_width: String,
    /// The `border-bottom-width` property. Not inherited.
    pub border_bottom_width: String,
    /// The `border-left-width` property. Not inherited.
    pub border_left_width: String,
    /// The `border-top-style` property. Not inherited.
    pub border_top_style: String,
    /// The `border-right-style` property. Not inherited.
    pub border_right_style: String,
    /// The `border-bottom-style` property. Not inherited.
    pub border_bottom_style: String,
    /// The `border-left-style` property. Not inherited.
    pub border_left_style: String,
    /// The `border-top-color` property. Not inherited; `currentcolor` by default.
    pub border_top_color: String,
    /// The `border-right-color` property. Not inherited.
    pub border_right_color: String,
    /// The `border-bottom-color` property. Not inherited.
    pub border_bottom_color: String,
    /// The `border-left-color` property. Not inherited.
    pub border_left_color: String,
    /// The `font-size` property. Inherited.
    pub font_size: String,
    /// The `font-weight` property. Inherited.
    pub font_weight: String,
    /// The `font-family` property. Inherited.
    ///
    /// The empty string means "the user agent default", because this engine has
    /// no UA stylesheet and no default font yet.
    pub font_family: String,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self::initial()
    }
}

impl ComputedStyle {
    /// A style with every string field empty and no allocations.
    ///
    /// Only the cascade uses this: [`crate::cascade::StyleEngine`] fills all
    /// [`PropertyId`](crate::cascade) slots immediately afterwards, so an empty
    /// style never escapes the crate. `empty()` exists so that the per-element
    /// hot path pays for each field exactly once instead of building
    /// [`ComputedStyle::initial`] and overwriting it.
    ///
    /// A field added to this struct must be added here *and* to the cascade's
    /// property table; the unit test
    /// `filling_every_property_from_initial_reproduces_the_initial_style`
    /// fails if the two drift apart.
    pub(crate) fn empty() -> Self {
        Self {
            display: DisplayType::Inline,
            color: String::new(),
            background_color: String::new(),
            width: String::new(),
            height: String::new(),
            margin_top: String::new(),
            margin_right: String::new(),
            margin_bottom: String::new(),
            margin_left: String::new(),
            padding_top: String::new(),
            padding_right: String::new(),
            padding_bottom: String::new(),
            padding_left: String::new(),
            border_top_width: String::new(),
            border_right_width: String::new(),
            border_bottom_width: String::new(),
            border_left_width: String::new(),
            border_top_style: String::new(),
            border_right_style: String::new(),
            border_bottom_style: String::new(),
            border_left_style: String::new(),
            border_top_color: String::new(),
            border_right_color: String::new(),
            border_bottom_color: String::new(),
            border_left_color: String::new(),
            font_size: String::new(),
            font_weight: String::new(),
            font_family: String::new(),
        }
    }

    /// The initial value of every tracked property.
    ///
    /// These are the CSS initial values, with three documented approximations:
    ///
    /// * `color` is `black` — CSS says `canvastext`, a system colour, and system
    ///   colours are not implemented.
    /// * `font-family` is the empty string, meaning "the user-agent default",
    ///   because there is no UA font stack yet.
    /// * `font-size` is `16px` rather than the keyword `medium`, because `medium`
    ///   computes to 16px under the standard default font settings and this crate
    ///   does not absolutise declared lengths yet. A declared `font-size: medium`
    ///   does stay the text `medium`; see [`crate::cascade`].
    #[must_use]
    pub fn initial() -> Self {
        Self {
            display: DisplayType::Inline,
            color: "black".to_owned(),
            background_color: "transparent".to_owned(),
            width: "auto".to_owned(),
            height: "auto".to_owned(),
            margin_top: "0".to_owned(),
            margin_right: "0".to_owned(),
            margin_bottom: "0".to_owned(),
            margin_left: "0".to_owned(),
            padding_top: "0".to_owned(),
            padding_right: "0".to_owned(),
            padding_bottom: "0".to_owned(),
            padding_left: "0".to_owned(),
            border_top_width: "0".to_owned(),
            border_right_width: "0".to_owned(),
            border_bottom_width: "0".to_owned(),
            border_left_width: "0".to_owned(),
            border_top_style: "none".to_owned(),
            border_right_style: "none".to_owned(),
            border_bottom_style: "none".to_owned(),
            border_left_style: "none".to_owned(),
            border_top_color: "currentcolor".to_owned(),
            border_right_color: "currentcolor".to_owned(),
            border_bottom_color: "currentcolor".to_owned(),
            border_left_color: "currentcolor".to_owned(),
            font_size: "16px".to_owned(),
            font_weight: "normal".to_owned(),
            font_family: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_keywords_round_trip() {
        for keyword in [
            "none",
            "contents",
            "block",
            "inline",
            "inline-block",
            "flow-root",
            "list-item",
            "flex",
            "inline-flex",
            "grid",
            "table",
            "table-row",
            "table-cell",
            "table-caption",
        ] {
            let display = DisplayType::from_keyword(keyword).expect("known keyword");
            assert_eq!(display.as_str(), keyword);
            assert_eq!(display.to_string(), keyword);
        }
    }

    #[test]
    fn display_keywords_ignore_ascii_case_and_surrounding_space() {
        assert_eq!(
            DisplayType::from_keyword("  BLOCK "),
            Some(DisplayType::Block)
        );
        // The CSS-wide keywords and multi-keyword values are not display types.
        for value in ["inherit", "initial", "unset", "inline flow-root", ""] {
            assert_eq!(DisplayType::from_keyword(value), None, "{value}");
        }
    }

    #[test]
    fn display_predicates() {
        assert!(DisplayType::None.is_none());
        assert!(DisplayType::Inline.is_inline());
        assert!(DisplayType::InlineFlex.is_inline());
        assert!(!DisplayType::Block.is_inline());
        assert!(!DisplayType::Flex.is_inline());
    }

    #[test]
    fn initial_style_uses_css_initial_values() {
        let style = ComputedStyle::initial();
        assert_eq!(style.display, DisplayType::Inline);
        assert_eq!(style.color, "black");
        assert_eq!(style.background_color, "transparent");
        assert_eq!(style.width, "auto");
        assert_eq!(style.margin_top, "0");
        assert_eq!(style.font_size, "16px");
        assert_eq!(style.font_weight, "normal");
        assert!(style.font_family.is_empty());
        assert_eq!(
            style,
            ComputedStyle::default(),
            "Default is the initial style"
        );
    }
}
