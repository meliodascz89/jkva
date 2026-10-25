//! Integration tests that exercise `jkva-paint` through its public API only.
//!
//! The unit tests inside the crate cover the edge cases of each type; these tests pin
//! the contract a caller in a later layer (`jkva-gfx`, `jkva-engine`) actually depends
//! on: build a layer, ask for its extents, and turn the extents into damage.

use jkva_paint::{
    Color, DEFAULT_MERGE_GAP, DamageRegion, DisplayItem, DisplayList, ESTIMATED_GLYPH_ASCENT_EM,
    GLYPH_ADVANCE_BUDGET_EM, Point, Rect, Size,
};

/// Asserts that two `f32` values are within a small epsilon.
fn assert_close(got: f32, want: f32) {
    assert!((got - want).abs() < 1.0e-4, "expected {want}, got {got}");
}

/// Asserts that two rectangles are within a small epsilon on every component.
fn assert_rect_close(got: Rect, want: Rect) {
    assert_close(got.origin.x, want.origin.x);
    assert_close(got.origin.y, want.origin.y);
    assert_close(got.size.w, want.size.w);
    assert_close(got.size.h, want.size.h);
}

/// Builds a layer that looks like a small card with a title and an image.
fn sample_layer() -> DisplayList {
    let mut list = DisplayList::new();
    list.push(DisplayItem::Rect {
        rect: Rect::new(0.0, 0.0, 800.0, 600.0),
        color: [250, 250, 250, 255],
    });
    list.push(DisplayItem::RoundedRect {
        rect: Rect::new(40.0, 40.0, 320.0, 200.0),
        radius: 12.0,
        color: [255, 255, 255, 255],
    });
    list.push(DisplayItem::Border {
        rect: Rect::new(40.0, 40.0, 320.0, 200.0),
        width: 1.0,
        color: [200, 200, 200, 255],
    });
    list.push(DisplayItem::GlyphRun {
        origin: Point::new(60.0, 80.0),
        text: "JKva".to_string(),
        size: 24.0,
        color: [20, 20, 20, 255],
    });
    list.push(DisplayItem::Image {
        rect: Rect::new(400.0, 40.0, 120.0, 90.0),
        source: "https://example.invalid/logo.png".to_string(),
    });
    list
}

#[test]
fn a_layer_is_ordered_and_its_bounds_cover_every_item() {
    let list = sample_layer();
    assert_eq!(list.len(), 5);
    assert!(!list.is_empty());

    // Paint order is insertion order: the background first, the image last.
    assert!(matches!(list.items[0], DisplayItem::Rect { .. }));
    assert!(matches!(list.items[4], DisplayItem::Image { .. }));

    // The background dominates the bounds of this layer.
    assert_eq!(list.bounds(), Rect::new(0.0, 0.0, 800.0, 600.0));
    // Every item is inside the layer bounds, which is the invariant a compositor
    // relies on when it scissors to `bounds()`.
    for item in &list {
        assert!(list.bounds().contains_rect(&item.bounds()));
    }
}

#[test]
fn glyph_bounds_match_the_documented_estimate() {
    let item = DisplayItem::GlyphRun {
        origin: Point::new(60.0, 80.0),
        text: "JKva".to_string(),
        size: 24.0,
        color: [0, 0, 0, 255],
    };
    // width  = chars * size * advance budget (one em per glyph)
    // top    = baseline - size * ascent ratio
    // height = one em
    let width = 4.0 * 24.0 * GLYPH_ADVANCE_BUDGET_EM;
    let top = 80.0 - 24.0 * ESTIMATED_GLYPH_ASCENT_EM;
    assert_rect_close(item.bounds(), Rect::new(60.0, top, width, 24.0));
    // The run sits above and below the baseline, not entirely above it.
    assert!(item.bounds().contains(Point::new(61.0, 79.0)));
}

#[test]
fn recomputing_only_the_changed_item_damages_only_its_area() {
    // This is the per-frame path the retained-layer design is built for: one item
    // changes, so only the union of its old and new bounds is damaged.
    let mut list = sample_layer();
    let before = list.items[3].bounds();

    list.items[3] = DisplayItem::GlyphRun {
        origin: Point::new(60.0, 80.0),
        text: "JKva!".to_string(),
        size: 24.0,
        color: [20, 20, 20, 255],
    };
    let after = list.items[3].bounds();

    let mut damage = DamageRegion::with_merge_gap(0.0);
    damage.add(before);
    damage.add(after);

    // The longer label grows to the right and overlaps its old position, so one
    // coalesced rectangle covers both.
    assert_eq!(damage.len(), 1);
    assert!(damage.bounds().contains_rect(&before));
    assert!(damage.bounds().contains_rect(&after));
    // Damage stays local: it must not reach the image on the other side of the layer.
    let image = list.items[4].bounds();
    assert!(damage.bounds().intersection(&image).is_empty());
    // ... and it is far smaller than the full layer.
    assert!(damage.bounds().width() < list.bounds().width() / 2.0);
}

#[test]
fn damage_merging_respects_the_configured_gap() {
    let near = Rect::new(0.0, 0.0, 10.0, 10.0);
    let other = Rect::new(14.0, 0.0, 10.0, 10.0);

    let mut tight = DamageRegion::with_merge_gap(2.0);
    tight.add(near);
    tight.add(other);
    assert_eq!(tight.len(), 2);

    let mut loose = DamageRegion::with_merge_gap(DEFAULT_MERGE_GAP);
    loose.add(near);
    loose.add(other);
    assert_eq!(loose.len(), 1);
}

#[test]
fn geometry_values_compose_without_surprises() {
    // A caller composes rectangles from layout output; the helpers must agree.
    let origin = Point::new(10.0, 20.0);
    let size = Size::new(30.0, 40.0);
    let from_parts = Rect::from_origin_size(origin, size);
    assert_eq!(from_parts, Rect::new(10.0, 20.0, 30.0, 40.0));
    assert_eq!(from_parts.min_x(), 10.0);
    assert_eq!(from_parts.max_y(), 60.0);
    assert_close(from_parts.width(), 30.0);

    let color: Color = [1, 2, 3, 4];
    let item = DisplayItem::Rect {
        rect: from_parts,
        color,
    };
    assert_eq!(item.bounds(), from_parts);
}
