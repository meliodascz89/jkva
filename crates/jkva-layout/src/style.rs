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

/// The subset of `display` values that M1 layout understands.
///
/// Everything else (`flex`, `grid`, `table-*`, `list-item`, `inline-block`,
/// `contents`, …) has to be mapped by the engine to one of these three for now; a
/// box that behaves like a block maps to [`Display::Block`], anything inline-level
/// maps to [`Display::Inline`]. M2 replaces this enum with the full display model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Display {
    /// `display: none` — the element and its subtree generate no boxes.
    None,
    /// `display: block` — the element is laid out in block flow.
    Block,
    /// `display: inline` — flattened into the inline flow of the nearest block.
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
    fn lengths_resolve_against_a_base() {
        assert_eq!(Length::Auto.resolve(Some(400.0)), None);
        assert_eq!(Length::px(12.0).resolve(None), Some(12.0));
        assert_eq!(Length::percent(50.0).resolve(Some(400.0)), Some(200.0));
        assert_eq!(Length::percent(50.0).resolve(None), None);
        assert_eq!(Length::px(-1.0).resolve(None), None);
    }
}
