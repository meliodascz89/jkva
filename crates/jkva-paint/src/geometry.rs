//! Geometry primitives shared by the paint layer.
//!
//! All coordinates are CSS pixels in the layer's local coordinate space: the origin
//! is the top-left corner, `x` grows to the right and `y` grows downwards, which is
//! the usual browser convention. Values are `f32` because that is what layout and the
//! GPU path use, and because damage maths needs sub-pixel precision.
//!
//! These types are a placeholder. ADR-017 keeps this crate independent of the layout
//! engine; when `jkva-layout` starts producing authoritative boxes, its rectangle type
//! replaces the local one and every parameter is preserved — the display-list shape
//! does not change. Until then this module is deliberately small and total: no
//! panics, no allocations.
//!
//! # Rectangle model
//!
//! A [`Rect`] is described by an [`origin`](Rect::origin) and a [`size`](Rect::size),
//! but every operation works on the box between the minimum and maximum corners, so a
//! rectangle whose `size` has negative components is still well defined. The
//! constructors [`Rect::new`] and [`Rect::from_origin_size`] additionally normalise
//! negative extents by moving the origin, so rectangles built through them never have
//! a negative `size`.
//!
//! Rectangles are **half-open**: a [`Rect`] covers
//! `[min_x, max_x) × [min_y, max_y)`. Two rectangles that only share an edge therefore
//! do not overlap, and a point on the right or bottom edge is not contained. That
//! matches how rasterisers fill pixels and keeps [`Rect::union`] and
//! [`Rect::intersection`] exact.
//!
//! An **empty** rectangle covers no pixels: this is what [`Rect::EMPTY`] represents.
//! `width <= 0`, `height <= 0` and NaN extents are all empty (see [`Rect::is_empty`]);
//! empty rectangles are the identity of [`Rect::union`] and absorb everything in
//! [`Rect::intersection`]. Damage tracking relies on that, so a degenerate item can
//! never inflate a dirty region.

/// An 8-bit RGBA colour with non-premultiplied (straight) alpha.
///
/// The channel order is `[red, green, blue, alpha]`, and each channel is a full
/// `0..=255` range. Colours are recorded exactly as the style system produced them,
/// assumed to be sRGB; converting into the compositor's working space is `jkva-gfx`'s
/// job, not the paint layer's.
///
/// Wider gamuts (Display P3) and HDR values are part of ADR-017 but need a richer
/// colour type; they are deliberately out of scope for this plain-data display list.
pub type Color = [u8; 4];

/// A position in layer space, in CSS pixels.
///
/// `x` grows to the right and `y` grows downwards from the layer's top-left corner.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    /// Horizontal coordinate, CSS pixels from the layer's left edge.
    pub x: f32,
    /// Vertical coordinate, CSS pixels from the layer's top edge.
    pub y: f32,
}

impl Point {
    /// The origin of layer space, `(0.0, 0.0)`.
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
#[derive(Debug, Clone, Copy, Default, PartialEq)]
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

/// An axis-aligned rectangle in layer space.
///
/// The rectangle is the box between its minimum and maximum corners; see the
/// [module documentation](self) for the half-open interval convention, how negative
/// extents are interpreted, and what "empty" means.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Top-left corner of the rectangle.
    pub origin: Point,
    /// Extents of the rectangle. Constructors keep these non-negative.
    pub size: Size,
}

impl Rect {
    /// A rectangle that covers nothing.
    ///
    /// This is the identity of [`Rect::union`] and the absorbing element of
    /// [`Rect::intersection`], so it is a safe default for "no damage yet".
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

    /// Returns `true` when the rectangle covers no pixels.
    ///
    /// A rectangle is empty when either extent is zero or negative, or when an extent
    /// is NaN (a NaN rectangle must never be painted or damaged, and NaN must not
    /// poison a union).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let w = self.width();
        let h = self.height();
        w <= 0.0 || h <= 0.0 || w.is_nan() || h.is_nan()
    }

    /// Returns the smallest rectangle that covers both `self` and `other`.
    ///
    /// An empty operand is ignored, so [`Rect::EMPTY`] is the identity of this
    /// operation: `Rect::EMPTY.union(&r) == r`. A NaN rectangle is empty, which means
    /// invalid geometry cannot widen a damage region.
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

    /// Returns the overlap of `self` and `other`, or [`Rect::EMPTY`] when they do not
    /// overlap.
    ///
    /// Rectangles that only share an edge do not overlap, because the covered
    /// intervals are half-open. If either operand is empty the result is empty.
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Self::EMPTY;
        }
        Self::from_min_max(
            self.min_x().max(other.min_x()),
            self.min_y().max(other.min_y()),
            self.max_x().min(other.max_x()),
            self.max_y().min(other.max_y()),
        )
    }

    /// Returns `true` when `point` lies inside the rectangle.
    ///
    /// The top and left edges are inside, the right and bottom edges are outside.
    /// An empty rectangle contains no point at all.
    #[must_use]
    pub fn contains(&self, point: Point) -> bool {
        if self.is_empty() {
            return false;
        }
        (self.min_x()..self.max_x()).contains(&point.x)
            && (self.min_y()..self.max_y()).contains(&point.y)
    }

    /// Returns `true` when `other` lies entirely inside `self`.
    ///
    /// Empty input is contained in everything, including another empty rectangle; an
    /// empty `self` contains only empty input. Rectangles may share edges.
    #[must_use]
    pub fn contains_rect(&self, other: &Self) -> bool {
        if other.is_empty() {
            return true;
        }
        if self.is_empty() {
            return false;
        }
        self.min_x() <= other.min_x()
            && other.max_x() <= self.max_x()
            && self.min_y() <= other.min_y()
            && other.max_y() <= self.max_y()
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
        assert_eq!(
            Rect::new(10.0, 10.0, 4.0, -6.0),
            Rect::new(10.0, 4.0, 4.0, 6.0)
        );
        let rect = Rect::new(10.0, 10.0, -4.0, -6.0);
        assert_eq!(
            (rect.origin, rect.size),
            (Point::new(6.0, 4.0), Size::new(4.0, 6.0))
        );
    }

    #[test]
    fn rect_from_origin_size_matches_rect_new() {
        let rect = Rect::from_origin_size(Point::new(5.0, 7.0), Size::new(11.0, 13.0));
        assert_eq!(rect, Rect::new(5.0, 7.0, 11.0, 13.0));
    }

    #[test]
    fn from_min_max_rejects_inverted_and_nan_corners() {
        assert_eq!(Rect::from_min_max(10.0, 10.0, 0.0, 0.0), Rect::EMPTY);
        assert_eq!(Rect::from_min_max(0.0, 0.0, 10.0, 0.0), Rect::EMPTY);
        assert_eq!(
            Rect::from_min_max(0.0, 0.0, 10.0, 10.0),
            Rect::new(0.0, 0.0, 10.0, 10.0)
        );
        assert!(Rect::from_min_max(f32::NAN, 0.0, 10.0, 10.0).is_empty());
    }

    #[test]
    fn empty_rect_is_the_default() {
        assert!(Rect::EMPTY.is_empty());
        assert_eq!(Rect::default(), Rect::EMPTY);
        assert_eq!(Rect::EMPTY.width(), 0.0);
        assert_eq!(Rect::EMPTY.height(), 0.0);
    }

    #[test]
    fn is_empty_detects_degenerate_extents() {
        assert!(Rect::new(0.0, 0.0, 0.0, 10.0).is_empty());
        assert!(Rect::new(0.0, 0.0, 10.0, 0.0).is_empty());
        assert!(!Rect::new(0.0, 0.0, 0.5, 0.5).is_empty());

        // A NaN extent is empty rather than "unknown", so it cannot poison a union.
        let nan = Rect::new(f32::NAN, 0.0, 10.0, 10.0);
        assert!(nan.is_empty());

        // A hand-built rectangle with a negative extent covers the same box as its
        // normalised counterpart and is not empty.
        let hand_built = Rect {
            origin: Point::new(10.0, 10.0),
            size: Size::new(-4.0, 6.0),
        };
        assert!(!hand_built.is_empty());
        assert_eq!(hand_built.min_x(), 6.0);
        assert_eq!(hand_built.max_x(), 10.0);
    }

    #[test]
    fn union_of_empty_operands_is_the_identity() {
        let rect = Rect::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(Rect::EMPTY.union(&rect), rect);
        assert_eq!(rect.union(&Rect::EMPTY), rect);
        assert_eq!(Rect::EMPTY.union(&Rect::EMPTY), Rect::EMPTY);

        let nan = Rect::new(f32::NAN, f32::NAN, 5.0, 5.0);
        assert_eq!(nan.union(&rect), rect);
        assert_eq!(rect.union(&nan), rect);
    }

    #[test]
    fn union_is_the_bounding_box_of_both_rects() {
        let a = Rect::new(10.0, 20.0, 30.0, 40.0);
        let b = Rect::new(50.0, 10.0, 10.0, 15.0);
        let union = a.union(&b);
        assert_eq!(union, Rect::new(10.0, 10.0, 50.0, 50.0));
        // The union is symmetric and idempotent.
        assert_eq!(union, b.union(&a));
        assert_eq!(union.union(&a), union);
        assert_eq!(union.union(&b), union);
        // Both operands are covered.
        assert!(union.contains_rect(&a));
        assert!(union.contains_rect(&b));
    }

    #[test]
    fn union_never_shrinks() {
        let a = Rect::new(-10.0, -10.0, 20.0, 20.0);
        let b = Rect::new(100.0, 100.0, 1.0, 1.0);
        let union = a.union(&b);
        assert!(union.contains_rect(&a));
        assert!(union.contains_rect(&b));
        assert!(union.width() >= a.width().max(b.width()));
        assert!(union.height() >= a.height().max(b.height()));
    }

    #[test]
    fn intersection_of_overlapping_rects() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 60.0, 100.0, 100.0);
        assert_eq!(a.intersection(&b), Rect::new(50.0, 60.0, 50.0, 40.0));
        // Symmetric.
        assert_eq!(a.intersection(&b), b.intersection(&a));
        // Contained rectangles intersect to themselves.
        let inner = Rect::new(10.0, 10.0, 5.0, 5.0);
        assert_eq!(a.intersection(&inner), inner);
    }

    #[test]
    fn intersection_of_disjoint_or_touching_rects_is_empty() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        // Separated on x.
        assert!(a.intersection(&Rect::new(11.0, 0.0, 10.0, 10.0)).is_empty());
        // Separated on y.
        assert!(a.intersection(&Rect::new(0.0, 20.0, 10.0, 10.0)).is_empty());
        // Sharing the right edge only: half-open intervals do not overlap.
        assert!(a.intersection(&Rect::new(10.0, 0.0, 10.0, 10.0)).is_empty());
        // Sharing the bottom edge only.
        assert!(a.intersection(&Rect::new(0.0, 10.0, 10.0, 10.0)).is_empty());
        // Sharing a corner only.
        assert!(a.intersection(&Rect::new(10.0, 10.0, 5.0, 5.0)).is_empty());
    }

    #[test]
    fn intersection_with_empty_is_empty() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert_eq!(a.intersection(&Rect::EMPTY), Rect::EMPTY);
        assert_eq!(Rect::EMPTY.intersection(&a), Rect::EMPTY);
    }

    #[test]
    fn contains_uses_half_open_intervals() {
        let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(rect.contains(Point::new(0.0, 0.0)));
        assert!(rect.contains(Point::new(9.999, 9.999)));
        assert!(!rect.contains(Point::new(10.0, 5.0)));
        assert!(!rect.contains(Point::new(5.0, 10.0)));
        assert!(!rect.contains(Point::new(-0.001, 5.0)));
        assert!(!rect.contains(Point::new(5.0, -0.001)));
        // Empty rectangles contain nothing, including their own origin.
        assert!(!Rect::EMPTY.contains(Point::ZERO));
        assert!(!Rect::new(0.0, 0.0, 0.0, 10.0).contains(Point::ZERO));
    }

    #[test]
    fn contains_rect_edge_cases() {
        let outer = Rect::new(0.0, 0.0, 10.0, 10.0);
        // An empty rectangle is contained in everything, including empty input.
        assert!(outer.contains_rect(&Rect::EMPTY));
        assert!(Rect::EMPTY.contains_rect(&Rect::EMPTY));
        // An empty container contains only empty input.
        assert!(!Rect::EMPTY.contains_rect(&outer));
        // Equality and edges are fine, a partial overlap is not.
        assert!(outer.contains_rect(&outer));
        assert!(outer.contains_rect(&Rect::new(0.0, 0.0, 10.0, 10.0)));
        assert!(!outer.contains_rect(&Rect::new(9.0, 0.0, 10.0, 10.0)));
        assert!(!outer.contains_rect(&Rect::new(-1.0, -1.0, 2.0, 2.0)));
    }

    #[test]
    fn min_max_accessors_normalise_hand_built_rects() {
        let rect = Rect {
            origin: Point::new(10.0, 20.0),
            size: Size::new(-4.0, -5.0),
        };
        assert_eq!(rect.min_x(), 6.0);
        assert_eq!(rect.max_x(), 10.0);
        assert_eq!(rect.min_y(), 15.0);
        assert_eq!(rect.max_y(), 20.0);
        assert_close(rect.width(), 4.0);
        assert_close(rect.height(), 5.0);

        // The covered area is the same as the normalised rectangle, but equality is
        // structural: the stored fields are never rewritten behind the caller's back.
        let normalised = Rect::new(6.0, 15.0, 4.0, 5.0);
        assert!(normalised.contains_rect(&rect));
        assert!(rect.contains_rect(&normalised));
        assert_ne!(rect, normalised);
    }
}
