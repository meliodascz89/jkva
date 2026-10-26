//! The styling contract that the layout engine consumes.
//!
//! Layout deliberately does not depend on `jkva-style`: the cascade is still moving,
//! and ADR-005 keeps the crate graph one-way and small. Instead layout asks a
//! [`StyleSource`] for the few values it needs. The engine implements this trait over
//! its computed style map and adapts the richer `jkva_style::DisplayType` to
//! [`Display`]; tests implement a mock.
//!
//! # Contract for implementors
//!
//! * Lengths are CSS pixels and already absolute. Layout never resolves `%`, `em`,
//!   `rem` or `calc()`: whoever implements this trait absolutises them, and an
//!   unresolvable declaration must arrive as `None` (auto) or `0.0`, never `NaN`.
//! * [`StyleSource::font_size`] and [`StyleSource::color`] are inherited properties.
//!   They must be resolved for the node that layout asks about, and layout also asks
//!   about **text nodes**, so inherited values must be propagated down to them.
//! * [`StyleSource::display`] is only queried for element nodes.
//! * [`StyleSource::background_color`] returns `None` for `transparent` and for
//!   elements that paint nothing.
//!
//! Layout clamps what it receives: negative and non-finite margins, paddings and
//! borders become `0.0`, and non-finite or negative sizes mean auto. A sloppy
//! implementation therefore cannot corrupt the box tree, but it should not rely on
//! that.

use jkva_dom::NodeId;

/// The `display` values that layout distinguishes.
///
/// Everything else (`grid`, `table-*`, `list-item`, `inline-block`, `contents`, …) has
/// to be mapped by the engine to one of these; a box that behaves like a block maps to
/// [`Display::Block`], anything inline-level maps to [`Display::Inline`]. Later
/// milestones replace this enum with the full display model.
///
/// `inline-flex` is **not** represented: the engine currently maps it to
/// [`Display::Inline`], so it is flattened into the surrounding inline flow instead of
/// becoming a flex container. Mapping it to [`Display::Flex`] would make it behave as a
/// block-level flex container, which is the documented fallback until inline-level flex
/// containers exist.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Display {
    /// `display: none` — the element and its subtree generate no boxes.
    None,
    /// `display: block` — the element is laid out in block flow.
    Block,
    /// `display: flex` — a block-level flex container.
    ///
    /// Its children become flex items and are laid out by the flex algorithm instead
    /// of block flow; see `jkva_layout::engine` for the modelled subset.
    Flex,
    /// `display: grid` — a block-level grid container.
    ///
    /// Its children are placed row-major into a column template; see
    /// `jkva_layout::engine` for the modelled subset.
    ///
    /// `inline-grid` is **not** represented: the engine currently maps it like an inline
    /// box, so it is flattened into the surrounding inline flow. Mapping it to
    /// [`Display::Grid`] would make it behave as a block-level grid container, which is
    /// the documented fallback until inline-level grid containers exist.
    Grid,
    /// `display: inline` — an inline box inside the nearest block's inline flow.
    ///
    /// This is the CSS initial value, and therefore the default.
    #[default]
    Inline,
}

/// A declared size: an absolute length, a percentage of the containing block, or auto.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    /// `auto` — the used size comes from the formatting context.
    #[default]
    Auto,
    /// An absolute length in CSS pixels.
    Px(f32),
    /// A percentage of the containing block's width (or height where definite).
    Percent(f32),
}

impl Length {
    /// Creates an absolute length.
    #[must_use]
    pub const fn px(value: f32) -> Self {
        Self::Px(value)
    }

    /// Creates a percentage.
    #[must_use]
    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    /// Resolves the length against `base`, or `None` when it cannot be resolved.
    ///
    /// Percentages need a definite base; negative and non-finite values resolve to
    /// `None` so they behave like `auto` instead of corrupting the box tree.
    #[must_use]
    pub fn resolve(self, base: Option<f32>) -> Option<f32> {
        let value = match self {
            Self::Auto => return None,
            Self::Px(value) => value,
            Self::Percent(percent) => base? * percent / 100.0,
        };
        (value.is_finite() && value >= 0.0).then_some(value)
    }
}

/// Margins or paddings where each side may be `auto`, a length or a percentage.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LengthEdges {
    /// Top edge.
    pub top: Length,
    /// Right edge.
    pub right: Length,
    /// Bottom edge.
    pub bottom: Length,
    /// Left edge.
    pub left: Length,
}

impl LengthEdges {
    /// Zero on every side.
    pub const ZERO: Self = Self {
        top: Length::Px(0.0),
        right: Length::Px(0.0),
        bottom: Length::Px(0.0),
        left: Length::Px(0.0),
    };

    /// Wraps absolute pixel edges, as used by test doubles.
    #[must_use]
    pub const fn from_edges(edges: Edges) -> Self {
        Self {
            top: Length::Px(edges.top),
            right: Length::Px(edges.right),
            bottom: Length::Px(edges.bottom),
            left: Length::Px(edges.left),
        }
    }

    /// Resolves every side against `base`; `auto` becomes `0.0` here.
    ///
    /// Horizontal centring is layout's decision (it needs the used width), so this
    /// conversion is deliberately dumb.
    #[must_use]
    pub fn resolve(self, base: Option<f32>) -> Edges {
        Edges {
            top: self.top.resolve(base).unwrap_or(0.0),
            right: self.right.resolve(base).unwrap_or(0.0),
            bottom: self.bottom.resolve(base).unwrap_or(0.0),
            left: self.left.resolve(base).unwrap_or(0.0),
        }
    }
}

/// The four margins, paddings or border widths of one box, in CSS pixels.
///
/// A single struct is used for margins, paddings and borders because layout treats
/// them identically apart from where they land: margins sit outside the border box,
/// padding and border sit inside it. Physical sides only — there is no
/// `margin-inline-start` resolution because RTL layout is out of scope (ADR-016).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    /// Top edge.
    pub top: f32,
    /// Right edge.
    pub right: f32,
    /// Bottom edge.
    pub bottom: f32,
    /// Left edge.
    pub left: f32,
}

impl Edges {
    /// Zero on every side.
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// Creates edges from the four sides, in CSS shorthand order.
    #[must_use]
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Creates edges with the same value on every side.
    #[must_use]
    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Sum of the left and right edges.
    #[must_use]
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Sum of the top and bottom edges.
    #[must_use]
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }

    /// Returns the edges with negative and non-finite values replaced by `0.0`.
    ///
    /// The layout algorithm only ever consumes this clamped form. A negative border
    /// or padding is invalid CSS and a negative margin is legal but not modelled yet
    /// (see the crate-level simplifications), so clamping keeps the box tree
    /// well-formed without a panic.
    #[must_use]
    pub(crate) fn clamped(&self) -> Self {
        Self {
            top: clamp(self.top),
            right: clamp(self.right),
            bottom: clamp(self.bottom),
            left: clamp(self.left),
        }
    }
}

/// Main axis of a flex container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexDirection {
    /// Items are laid out left to right.
    #[default]
    Row,
    /// Items are laid out top to bottom.
    Column,
}

/// Distribution of free space along the main axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JustifyContent {
    /// Pack items at the start.
    #[default]
    FlexStart,
    /// Pack items at the end.
    FlexEnd,
    /// Centre the items.
    Center,
    /// Equal space between items, none at the edges.
    SpaceBetween,
    /// Equal space around every item.
    SpaceAround,
}

/// Alignment of items on the cross axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignItems {
    /// Stretch items to the container's cross size.
    #[default]
    Stretch,
    /// Align at the cross-start edge.
    FlexStart,
    /// Centre on the cross axis.
    Center,
    /// Align at the cross-end edge.
    FlexEnd,
}

/// Whether a flex container puts its items on more than one line.
///
/// Wrapping is implemented for [`FlexDirection::Row`]; a column container ignores it and
/// stays single-line (see the crate-level simplifications).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexWrap {
    /// Keep every item on one line and let the line overflow (the initial value).
    #[default]
    NoWrap,
    /// Move items that do not fit onto a new line.
    Wrap,
}

/// Distribution of a multi-line flex container's lines along its cross axis.
///
/// Only a row container with more than one line and a definite cross size (a declared
/// height) uses it; a single line keeps its own cross size, and a column container is
/// single-line by definition (see the crate-level simplifications).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignContent {
    /// Pack the lines at the cross-start edge.
    FlexStart,
    /// Pack the lines at the cross-end edge.
    FlexEnd,
    /// Centre the lines.
    Center,
    /// Equal space between lines, none at the edges.
    SpaceBetween,
    /// Equal space around every line.
    SpaceAround,
    /// Grow every line so the lines fill the container (the initial value of
    /// `align-content` for a flex container).
    #[default]
    Stretch,
}

/// One column track of a simple grid template.
///
/// Produced by [`parse_grid_template_columns`] from the cascade's text. The grammar is
/// deliberately small: `Npx`, `Nfr`, `auto` and `N%` separated by whitespace. There is no
/// `repeat()`, no `minmax()`, no named lines and no `fit-content()`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridTrack {
    /// A fixed track, in CSS pixels.
    Px(f32),
    /// A share of the space left after the fixed, percentage and auto tracks, by factor.
    Fr(f32),
    /// As wide as the widest item's max-content contribution in that column.
    Auto,
    /// A percentage of the grid container's content width.
    Percent(f32),
}

/// How the implicit rows of a grid are sized.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum GridAutoRows {
    /// Every row is this many CSS pixels tall.
    Px(f32),
    /// Every row is as tall as its tallest item (the initial value).
    #[default]
    Auto,
}

/// Parses a `grid-template-columns` value into column tracks.
///
/// The grammar is a whitespace-separated list of `Npx`, `Nfr`, `auto` and `N%`. Tokens
/// that do not parse are skipped; a value that uses an unsupported construct
/// (`repeat(`, `minmax(`, `fit-content(`, `[` named lines) or is `none`/empty yields no
/// tracks at all, which layout treats as one implicit column.
///
/// Negative, non-finite and non-numeric sizes are ignored, `0fr` is a valid (zero-width)
/// track, and the caller can never get a track that would corrupt the column geometry.
#[must_use]
pub fn parse_grid_template_columns(value: &str) -> Vec<GridTrack> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        return Vec::new();
    }
    // These constructs change the shape of the template, so a partial parse would be
    // worse than no template at all.
    if ["repeat(", "minmax(", "fit-content(", "["]
        .iter()
        .any(|unsupported| trimmed.contains(unsupported))
    {
        return Vec::new();
    }
    trimmed
        .split_whitespace()
        .filter_map(parse_grid_track)
        .collect()
}

/// Parses one track token, or `None` when it is not part of the small grammar.
fn parse_grid_track(token: &str) -> Option<GridTrack> {
    let lower = token.to_ascii_lowercase();
    if lower == "auto" {
        return Some(GridTrack::Auto);
    }
    if let Some(number) = lower.strip_suffix("px") {
        return Some(GridTrack::Px(parse_track_number(number)?));
    }
    if let Some(number) = lower.strip_suffix("fr") {
        return Some(GridTrack::Fr(parse_track_number(number)?));
    }
    if let Some(number) = lower.strip_suffix('%') {
        return Some(GridTrack::Percent(parse_track_number(number)?));
    }
    None
}

/// Parses a non-negative, finite track number.
fn parse_track_number(number: &str) -> Option<f32> {
    let value: f32 = number.trim().parse().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

/// Parses a `grid-auto-rows` value.
///
/// Accepts a single `Npx` length or `auto`; everything else (including a track list,
/// which the property does not take here) falls back to [`GridAutoRows::Auto`].
#[must_use]
pub fn parse_grid_auto_rows(value: &str) -> GridAutoRows {
    let trimmed = value.trim().to_ascii_lowercase();
    if trimmed == "auto" {
        return GridAutoRows::Auto;
    }
    match trimmed.strip_suffix("px").and_then(parse_track_number) {
        Some(px) => GridAutoRows::Px(px),
        None => GridAutoRows::Auto,
    }
}

/// Parses a declared CSS `order` value.
///
/// The cascade keeps property values as text, so whoever implements
/// [`StyleSource::order`] parses the integer with `i32::from_str` through this helper.
/// Anything that is not an `i32` (`auto`, a length, an empty string) is treated as the
/// initial value `0`, which is what a browser does with an invalid `order` after the
/// declaration is dropped.
#[must_use]
pub fn parse_order(value: &str) -> i32 {
    value.trim().parse().unwrap_or(0)
}

/// Replaces negative and non-finite values with `0.0`.
///
/// `f32::max` returns the non-NaN operand, so `NaN.max(0.0)` is `0.0`; the explicit
/// check documents the intent and keeps infinities out of the box tree too.
fn clamp(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

/// The computed style that layout reads, resolved by the caller.
///
/// See the [module documentation](self) for the contract. Every method takes the
/// [`NodeId`] of the node being laid out; the implementation is
/// expected to be a cheap lookup (a map for tests, an index into the computed style
/// array in the engine).
///
/// # Implementations
///
/// The trait is object safe on purpose: [`layout`](crate::layout) takes
/// `&dyn StyleSource`, so the engine can hand over the cascade's result without a
/// monomorphised wrapper. Implementations must be pure with respect to layout: the
/// same node must always report the same value during one layout pass, because
/// layout calls each getter more than once and does not cache the answers.
pub trait StyleSource {
    /// Returns the `display` value of an element.
    ///
    /// Only called for elements; text nodes are always inline-level content.
    fn display(&self, node: NodeId) -> Display;

    /// Returns the declared content width.
    ///
    /// The value is a **content-box** width: padding and border are added on top by
    /// layout, matching the CSS `box-sizing: content-box` default. Percentages are
    /// resolved by layout against the containing block's width; `min-width` and
    /// `max-width` are not modelled yet.
    fn width(&self, node: NodeId) -> Length;

    /// Returns the declared content height.
    ///
    /// Like [`StyleSource::width`] this is a content-box height. A percentage height
    /// resolves against a definite containing height; M1 has no definite parent
    /// height, so percentages behave like `auto`. An explicit height wins over the
    /// content, so content overflows; overflow does not exist in M1.
    fn height(&self, node: NodeId) -> Length;

    /// Returns the declared margins; layout resolves them against the containing width.
    ///
    /// Percentage margins are relative to the containing block's **width**, as CSS
    /// requires, and `auto` on a horizontal side participates in centring.
    fn margin(&self, node: NodeId) -> LengthEdges;

    /// Returns the declared paddings; percentages resolve against the containing width.
    fn padding(&self, node: NodeId) -> LengthEdges;

    /// Returns the border widths of a box in CSS pixels.
    ///
    /// Only the widths matter: colours and styles are paint's business. A zero width
    /// on a side means "no border", which is all layout needs to know.
    fn border(&self, node: NodeId) -> Edges;

    /// Returns the used font size in CSS pixels. Inherited.
    ///
    /// Layout asks text nodes too, so the implementation must resolve inheritance
    /// down to every text node it expects to see laid out.
    fn font_size(&self, node: NodeId) -> f32;

    /// Returns the used text colour as straight RGBA. Inherited.
    ///
    /// Layout copies this into the text run and never interprets it; the colour is
    /// assumed to be sRGB, as in `jkva-paint`. Layout asks text nodes too.
    fn color(&self, node: NodeId) -> [u8; 4];

    /// Returns the flex main axis for a container. Only read when the display is
    /// [`Display::Flex`].
    fn flex_direction(&self, _node: NodeId) -> FlexDirection {
        FlexDirection::Row
    }

    /// Returns how free space is distributed along the flex main axis.
    fn justify_content(&self, _node: NodeId) -> JustifyContent {
        JustifyContent::FlexStart
    }

    /// Returns the cross-axis alignment of flex items.
    fn align_items(&self, _node: NodeId) -> AlignItems {
        AlignItems::Stretch
    }

    /// Returns the item's own cross-axis alignment, or `None` to follow the
    /// container's `align-items`.
    fn align_self(&self, _node: NodeId) -> Option<AlignItems> {
        None
    }

    /// Returns whether a flex container wraps its items onto more than one line.
    ///
    /// Defaults to [`FlexWrap::NoWrap`]. Only row containers honour it; a column
    /// container ignores wrapping.
    fn flex_wrap(&self, _node: NodeId) -> FlexWrap {
        FlexWrap::NoWrap
    }

    /// Returns the `flex-grow` factor of a flex item.
    ///
    /// Defaults to `0.0` (an item does not grow). Negative and non-finite values are
    /// clamped to `0.0` by layout, so an implementation cannot corrupt the line.
    /// Only a row container's main axis grows.
    fn flex_grow(&self, _node: NodeId) -> f32 {
        0.0
    }

    /// Returns the `flex-shrink` factor of a flex item.
    ///
    /// Defaults to `1.0` (CSS's initial value: an item shrinks when its line
    /// overflows). Negative and non-finite values are clamped to `0.0` by layout.
    fn flex_shrink(&self, _node: NodeId) -> f32 {
        1.0
    }

    /// Returns the declared `flex-basis` of a flex item.
    ///
    /// [`Length::Auto`] means "no basis": layout falls back to the declared width and
    /// then to the max-content approximation. A percentage resolves against the flex
    /// container's content width. Only a row container reads it.
    fn flex_basis(&self, _node: NodeId) -> Length {
        Length::Auto
    }

    /// Returns the item's `order`, defaulting to `0`.
    ///
    /// Flex items are stably sorted by this value before they are sized, wrapped and
    /// painted, so equal orders keep document order. An implementation reads the
    /// cascade's text and parses it with [`parse_order`].
    fn order(&self, _node: NodeId) -> i32 {
        0
    }

    /// Returns how a multi-line flex container distributes its lines.
    ///
    /// Defaults to [`AlignContent::Stretch`], the CSS initial value for a flex
    /// container. Layout only reads it for a row container with more than one line and a
    /// definite cross size; everywhere else the value is ignored.
    fn align_content(&self, _node: NodeId) -> AlignContent {
        AlignContent::Stretch
    }

    /// Returns the parsed `grid-template-columns` of a grid container.
    ///
    /// The cascade keeps the property as text; an implementation parses it with
    /// [`parse_grid_template_columns`]. An empty result means "no explicit template", and
    /// layout then uses one implicit column that fills the container width.
    fn grid_template_columns(&self, _node: NodeId) -> Vec<GridTrack> {
        Vec::new()
    }

    /// Returns how the implicit rows of a grid are sized.
    ///
    /// Defaults to [`GridAutoRows::Auto`], where every row is as tall as its tallest
    /// item; parse the cascade's text with [`parse_grid_auto_rows`].
    fn grid_auto_rows(&self, _node: NodeId) -> GridAutoRows {
        GridAutoRows::Auto
    }

    /// Returns the flex and grid gap in CSS pixels, applied to both axes.
    ///
    /// The same value separates items on a line and lines from each other, and grid
    /// columns and rows from each other.
    fn gap(&self, _node: NodeId) -> f32 {
        0.0
    }

    /// Returns the background colour of a box as straight RGBA, or `None` for
    /// `transparent`.
    ///
    /// This getter is part of the contract for the paint handoff: it is not inherited,
    /// and the current layout pass does not read it because [`LayoutBox`] carries no
    /// colour yet. It is here so that the engine's adapter is complete and the paint
    /// layer can ask the same source for the colour of a box without layout having to
    /// grow a field it will not use.
    ///
    /// [`LayoutBox`]: crate::LayoutBox
    fn background_color(&self, node: NodeId) -> Option<[u8; 4]>;

    /// Returns the font weight for a node. Inherited, resolved for text nodes too.
    ///
    /// Defaults to `400` so test doubles and simple adapters do not have to
    /// implement it; the engine overrides it with the cascade value.
    fn font_weight(&self, _node: NodeId) -> u16 {
        400
    }

    /// Returns the font family for a node; the empty string means the UA default.
    ///
    /// Defaults to empty for the same reason as [`StyleSource::font_weight`].
    fn font_family(&self, _node: NodeId) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_defaults_to_the_css_initial_value() {
        assert_eq!(Display::default(), Display::Inline);
        assert_ne!(Display::default(), Display::Block);
        assert_ne!(Display::Block, Display::None);
    }

    #[test]
    fn edges_constructors_and_sums() {
        assert_eq!(Edges::ZERO, Edges::new(0.0, 0.0, 0.0, 0.0));
        assert_eq!(Edges::all(2.5), Edges::new(2.5, 2.5, 2.5, 2.5));
        let edges = Edges::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(edges.horizontal(), 6.0);
        assert_eq!(edges.vertical(), 4.0);
        assert_eq!(Edges::default(), Edges::ZERO);
    }

    #[test]
    fn clamped_edges_remove_negative_and_non_finite_values() {
        let edges = Edges::new(-1.0, f32::NAN, f32::INFINITY, 7.0);
        assert_eq!(edges.clamped(), Edges::new(0.0, 0.0, 0.0, 7.0));
        assert_eq!(Edges::ZERO.clamped(), Edges::ZERO);
        assert_eq!(Edges::all(-0.0).clamped(), Edges::ZERO);
    }

    #[test]
    fn clamp_keeps_positive_finite_values() {
        assert_eq!(clamp(0.0), 0.0);
        assert_eq!(clamp(0.5), 0.5);
        assert_eq!(clamp(123.0), 123.0);
        assert_eq!(clamp(-0.001), 0.0);
        assert_eq!(clamp(f32::NEG_INFINITY), 0.0);
        assert_eq!(clamp(f32::NAN), 0.0);
    }

    #[test]
    fn flex_property_defaults_match_the_css_initial_values() {
        assert_eq!(FlexWrap::default(), FlexWrap::NoWrap);
        assert_ne!(FlexWrap::Wrap, FlexWrap::NoWrap);
        // The trait defaults are the initial values; an adapter that does not override
        // them must behave like an unstyled item.
        struct Unstyled;
        impl StyleSource for Unstyled {
            fn display(&self, _node: NodeId) -> Display {
                Display::Block
            }
            fn width(&self, _node: NodeId) -> Length {
                Length::Auto
            }
            fn height(&self, _node: NodeId) -> Length {
                Length::Auto
            }
            fn margin(&self, _node: NodeId) -> LengthEdges {
                LengthEdges::ZERO
            }
            fn padding(&self, _node: NodeId) -> LengthEdges {
                LengthEdges::ZERO
            }
            fn border(&self, _node: NodeId) -> Edges {
                Edges::ZERO
            }
            fn font_size(&self, _node: NodeId) -> f32 {
                16.0
            }
            fn color(&self, _node: NodeId) -> [u8; 4] {
                [0, 0, 0, 255]
            }
            fn background_color(&self, _node: NodeId) -> Option<[u8; 4]> {
                None
            }
        }
        // A style source is a pure lookup, so one probe call per getter is enough.
        let source = Unstyled;
        let node = jkva_dom::Document::new().root();
        assert_eq!(source.flex_wrap(node), FlexWrap::NoWrap);
        assert_eq!(source.flex_grow(node), 0.0);
        assert_eq!(source.flex_shrink(node), 1.0);
        assert_eq!(source.flex_basis(node), Length::Auto);
        assert_eq!(source.order(node), 0);
        assert_eq!(source.align_content(node), AlignContent::Stretch);
    }

    #[test]
    fn grid_template_columns_parse_the_small_grammar() {
        assert_eq!(
            parse_grid_template_columns("60px 1fr auto 25%"),
            vec![
                GridTrack::Px(60.0),
                GridTrack::Fr(1.0),
                GridTrack::Auto,
                GridTrack::Percent(25.0),
            ]
        );
        // Case-insensitive units and surrounding whitespace.
        assert_eq!(
            parse_grid_template_columns(" 2FR  0fr AUTO "),
            vec![GridTrack::Fr(2.0), GridTrack::Fr(0.0), GridTrack::Auto]
        );
        // `none`, empty and unsupported constructs yield no template.
        for value in [
            "",
            "   ",
            "none",
            "NONE",
            "repeat(3, 1fr)",
            "minmax(100px, 1fr)",
            "fit-content(20%)",
            "[a] 1fr",
        ] {
            assert!(parse_grid_template_columns(value).is_empty(), "{value}");
        }
        // A partial template skips the tokens it cannot read.
        assert_eq!(
            parse_grid_template_columns("60px nonsense 1fr"),
            vec![GridTrack::Px(60.0), GridTrack::Fr(1.0)]
        );
        // Negative and non-finite tracks are ignored.
        assert!(parse_grid_template_columns("-10px").is_empty());
        assert!(parse_grid_template_columns("infpx").is_empty());
    }

    #[test]
    fn grid_auto_rows_parses_a_single_length_or_auto() {
        assert_eq!(parse_grid_auto_rows("auto"), GridAutoRows::Auto);
        assert_eq!(parse_grid_auto_rows(" 40PX "), GridAutoRows::Px(40.0));
        assert_eq!(parse_grid_auto_rows("0px"), GridAutoRows::Px(0.0));
        // Anything else keeps the initial value.
        assert_eq!(parse_grid_auto_rows("40px 60px"), GridAutoRows::Auto);
        assert_eq!(parse_grid_auto_rows("-5px"), GridAutoRows::Auto);
        assert_eq!(parse_grid_auto_rows("nonsense"), GridAutoRows::Auto);
        assert_eq!(parse_grid_auto_rows(""), GridAutoRows::Auto);
        assert_eq!(GridAutoRows::default(), GridAutoRows::Auto);
    }

    #[test]
    fn align_content_defaults_to_stretch_and_order_parses_integers() {
        assert_eq!(AlignContent::default(), AlignContent::Stretch);
        assert_ne!(AlignContent::FlexStart, AlignContent::Stretch);
        // The cascade keeps the value as text; layout parses it here.
        assert_eq!(parse_order("0"), 0);
        assert_eq!(parse_order("3"), 3);
        assert_eq!(parse_order(" -2 "), -2);
        assert_eq!(parse_order("+4"), 4);
        // Invalid values fall back to the initial value instead of panicking.
        assert_eq!(parse_order("auto"), 0);
        assert_eq!(parse_order("3px"), 0);
        assert_eq!(parse_order(""), 0);
        assert_eq!(parse_order("99999999999999999999"), 0);
    }

    #[test]
    fn lengths_resolve_against_a_base() {
        assert_eq!(Length::Auto.resolve(Some(400.0)), None);
        assert_eq!(Length::px(12.0).resolve(None), Some(12.0));
        assert_eq!(Length::percent(50.0).resolve(Some(400.0)), Some(200.0));
        assert_eq!(Length::percent(50.0).resolve(None), None);
        assert_eq!(Length::px(-1.0).resolve(None), None);
    }
}
