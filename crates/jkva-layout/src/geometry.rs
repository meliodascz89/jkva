//! Geometry primitives used by the layout engine.
//!
//! Coordinates are CSS pixels in the document coordinate space: the origin is the
//! top-left corner of the page, `x` grows to the right and `y` grows downwards, which
//! is the usual browser convention. Values are `f32` because that is what the rest of
//! the engine uses for layout and painting.
//!
//! These types are deliberately **copies**, not shared ones. `jkva-paint` keeps its own
//! [`Rect`](https://docs.rs/jkva-paint) for the same reason (ADR-017 keeps the paint
//! layer independent of layout), and `jkva-gfx` will eventually hold a third. The
//! shapes are kept identical so that moving between layers is a field-by-field copy,
//! and layout must not depend on a rendering crate to state where a box is.
//!
//! # Rectangle model
//!
//! A [`Rect`] stores an [`origin`](Rect::origin) and a [`size`](Rect::size). Every
//! operation works on the box between the minimum and maximum corners, so a
//! rectangle whose `size` has negative components is still well defined.
//! [`Rect::new`] and [`Rect::from_origin_size`] additionally normalise negative
//! extents by moving the origin, so rectangles built through them never have a
//! negative `size`.
//!
//! Rectangles are **half-open**: a [`Rect`] covers `[min_x, max_x) × [min_y, max_y)`.
//! Two rectangles that only share an edge therefore do not overlap.
//!
//! An **empty** rectangle covers no area: this is what [`Rect::EMPTY`] represents.
//! `width <= 0`, `height <= 0` and NaN extents are all empty (see [`Rect::is_empty`]);
//! empty rectangles are the identity of [`Rect::union`]. Layout never relies on empty
//! rectangles for control flow, but consumers such as damage tracking do.

/// A position in document space, in CSS pixels.
///
/// `x` grows to the right and `y` grows downwards from the top-left corner of the page.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal coordinate, CSS pixels from the left edge of the page.
    pub x: f32,
    /// Vertical coordinate, CSS pixels from the top edge of the page.
    pub y: f32,
}

impl Point {
    /// The origin of document space, `(0.0, 0.0)`.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Creates a point from its coordinates.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A width and a height, in CSS pixels.
///
/// A [`Size`] is a pair of extents, not a rectangle: it carries no position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    /// Horizontal extent, CSS pixels.
    pub w: f32,
    /// Vertical extent, CSS pixels.
    pub h: f32,
}

impl Size {
    /// A zero extent, `(0.0, 0.0)`.
    pub const ZERO: Self = Self { w: 0.0, h: 0.0 };

    /// Creates a size from its extents.
    #[must_use]
    pub const fn new(w: f32, h: f32) -> Self {
        Self { w, h }
    }
}

/// An axis-aligned rectangle in document space.
///
/// The rectangle is the box between its minimum and maximum corners; see the
/// [module documentation](self) for the half-open interval convention, how negative
/// extents are interpreted, and what "empty" means.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// Top-left corner of the rectangle.
    pub origin: Point,
    /// Extents of the rectangle. Constructors keep these non-negative.
    pub size: Size,
}

impl Rect {
    /// A rectangle that covers nothing.
    ///
    /// This is the identity of [`Rect::union`], so it is a safe default for "this box
    /// is not positioned yet".
    pub const EMPTY: Self = Self {
        origin: Point::ZERO,
        size: Size::ZERO,
    };

    /// Creates a rectangle from a corner and extents.
    ///
    /// A negative `w` or `h` is normalised by moving the origin so that the stored
    /// `size` is non-negative; for example `new(10.0, 10.0, -4.0, 6.0)` becomes the
    /// box `(6.0, 10.0)`–`(10.0, 16.0)`.
    #[must_use]
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        let (x, w) = if w < 0.0 { (x + w, -w) } else { (x, w) };
        let (y, h) = if h < 0.0 { (y + h, -h) } else { (y, h) };
        Self {
            origin: Point { x, y },
            size: Size { w, h },
        }
    }

    /// Creates a rectangle from an origin and extents.
    ///
    /// Extents are normalised exactly like [`Rect::new`].
    #[must_use]
    pub const fn from_origin_size(origin: Point, size: Size) -> Self {
        Self::new(origin.x, origin.y, size.w, size.h)
    }

    /// Creates the rectangle spanned by two opposite corners.
    ///
    /// Returns [`Rect::EMPTY`] when the maximum corner is not strictly greater than
    /// the minimum corner on both axes, which includes inverted and NaN input. This
    /// is the constructor the set operations use, because it cannot produce a
    /// negative-extent rectangle.
    #[must_use]
    pub const fn from_min_max(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        let w = max_x - min_x;
        let h = max_y - min_y;
        // `<= 0` and NaN both mean "covers nothing"; see `is_empty`.
        if w <= 0.0 || h <= 0.0 || w.is_nan() || h.is_nan() {
            return Self::EMPTY;
        }
        Self {
            origin: Point { x: min_x, y: min_y },
            size: Size { w, h },
        }
    }

    /// Left edge, i.e. the smaller of the two horizontal corners.
    ///
    /// Unlike [`Rect::origin`] this is meaningful for hand-built rectangles with a
    /// negative extent, where the stored origin is not the top-left corner.
    #[must_use]
    pub fn min_x(&self) -> f32 {
        self.origin.x.min(self.origin.x + self.size.w)
    }

    /// Top edge, i.e. the smaller of the two vertical corners.
    #[must_use]
    pub fn min_y(&self) -> f32 {
        self.origin.y.min(self.origin.y + self.size.h)
    }

    /// Right edge, i.e. the larger of the two horizontal corners.
    #[must_use]
    pub fn max_x(&self) -> f32 {
        self.origin.x.max(self.origin.x + self.size.w)
    }

    /// Bottom edge, i.e. the larger of the two vertical corners.
    #[must_use]
    pub fn max_y(&self) -> f32 {
        self.origin.y.max(self.origin.y + self.size.h)
    }

    /// Horizontal extent between the two corners. Never negative.
    #[must_use]
    pub fn width(&self) -> f32 {
        self.max_x() - self.min_x()
    }

    /// Vertical extent between the two corners. Never negative.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.max_y() - self.min_y()
    }

    /// Returns `true` when the rectangle covers no area.
    ///
    /// A rectangle is empty when either extent is zero or negative, or when an extent
    /// is NaN. Layout does not create NaN extents itself, but a consumer that unions
    /// boxes must not have its result poisoned by one.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let w = self.width();
        let h = self.height();
        w <= 0.0 || h <= 0.0 || w.is_nan() || h.is_nan()
    }

    /// Returns the smallest rectangle that covers both `self` and `other`.
    ///
    /// An empty operand is ignored, so [`Rect::EMPTY`] is the identity of this
    /// operation: `Rect::EMPTY.union(&r) == r`.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        Self::from_min_max(
            self.min_x().min(other.min_x()),
            self.min_y().min(other.min_y()),
            self.max_x().max(other.max_x()),
            self.max_y().max(other.max_y()),
        )
    }
}

impl Default for Rect {
    /// An empty rectangle, identical to [`Rect::EMPTY`].
    fn default() -> Self {
        Self::EMPTY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asserts that two `f32` values are within a small epsilon.
    fn assert_close(got: f32, want: f32) {
        assert!((got - want).abs() < 1.0e-6, "expected {want}, got {got}");
    }

    #[test]
    fn point_and_size_constructors_keep_their_values() {
        assert_eq!(Point::new(1.5, -2.5), Point { x: 1.5, y: -2.5 });
        assert_eq!(Size::new(3.0, 4.0), Size { w: 3.0, h: 4.0 });
        assert_eq!(Point::ZERO, Point { x: 0.0, y: 0.0 });
        assert_eq!(Size::ZERO, Size { w: 0.0, h: 0.0 });
        assert_eq!(Point::default(), Point::ZERO);
        assert_eq!(Size::default(), Size::ZERO);
    }

    #[test]
    fn rect_new_normalises_negative_extents() {
        assert_eq!(
            Rect::new(10.0, 10.0, -4.0, 6.0),
            Rect::new(6.0, 10.0, 4.0, 6.0)
        );
        let rect = Rect::new(10.0, 10.0, -4.0, -6.0);
        assert_eq!(
            (rect.origin, rect.size),
            (Point::new(6.0, 4.0), Size::new(4.0, 6.0))
        );
        let rect = Rect::from_origin_size(Point::new(5.0, 7.0), Size::new(11.0, 13.0));
        assert_eq!(rect, Rect::new(5.0, 7.0, 11.0, 13.0));
    }

    #[test]
    fn empty_rect_is_the_default_and_has_no_extent() {
        assert!(Rect::EMPTY.is_empty());
        assert_eq!(Rect::default(), Rect::EMPTY);
        assert_eq!(Rect::EMPTY.width(), 0.0);
        assert_eq!(Rect::EMPTY.height(), 0.0);
    }

    #[test]
    fn degenerate_extents_are_empty() {
        assert!(Rect::new(0.0, 0.0, 0.0, 10.0).is_empty());
        assert!(Rect::new(0.0, 0.0, 10.0, 0.0).is_empty());
        assert!(!Rect::new(0.0, 0.0, 0.5, 0.5).is_empty());
        // NaN must not poison a union.
        assert!(Rect::new(f32::NAN, 0.0, 10.0, 10.0).is_empty());
        // A hand-built negative extent still covers the same box.
        let hand_built = Rect {
            origin: Point::new(10.0, 10.0),
            size: Size::new(-4.0, 6.0),
        };
        assert!(!hand_built.is_empty());
        assert_close(hand_built.min_x(), 6.0);
        assert_close(hand_built.max_x(), 10.0);
    }

    #[test]
    fn from_min_max_rejects_inverted_and_nan_corners() {
        assert_eq!(Rect::from_min_max(10.0, 10.0, 0.0, 0.0), Rect::EMPTY);
        assert_eq!(Rect::from_min_max(0.0, 0.0, 10.0, 0.0), Rect::EMPTY);
        assert!(Rect::from_min_max(f32::NAN, 0.0, 10.0, 10.0).is_empty());
        assert_eq!(
            Rect::from_min_max(0.0, 0.0, 10.0, 10.0),
            Rect::new(0.0, 0.0, 10.0, 10.0)
        );
    }

    #[test]
    fn union_of_empty_operands_is_the_identity() {
        let rect = Rect::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(Rect::EMPTY.union(&rect), rect);
        assert_eq!(rect.union(&Rect::EMPTY), rect);
        assert_eq!(Rect::EMPTY.union(&Rect::EMPTY), Rect::EMPTY);
        let nan = Rect::new(f32::NAN, f32::NAN, 5.0, 5.0);
        assert_eq!(nan.union(&rect), rect);
    }

    #[test]
    fn union_is_the_bounding_box_of_both_rects() {
        let a = Rect::new(10.0, 20.0, 30.0, 40.0);
        let b = Rect::new(50.0, 10.0, 10.0, 15.0);
        let union = a.union(&b);
        assert_eq!(union, Rect::new(10.0, 10.0, 50.0, 50.0));
        assert_eq!(union, b.union(&a));
        assert_eq!(union.union(&a), union);
        assert_close(union.width(), 50.0);
        assert_close(union.height(), 50.0);
    }
}
