//! Display lists: an ordered, backend-agnostic record of paint operations.
//!
//! A [`DisplayList`] is the fluent output of the paint stage and the input of the
//! recorder in `jkva-gfx`. It is *plain data*: no GPU handles, no fonts, no image
//! bitmaps, no side effects. That keeps the list cheap to retain per layer, cheap to
//! diff, and testable without a device.
//!
//! # Paint order
//!
//! Items are painted in list order: index `0` is painted first and therefore ends up
//! at the bottom, and the last item is painted on top. Ordering is the only thing that
//! decides what covers what in this version — stacking contexts, z-index, clipping and
//! transforms are follow-up work from ADR-017 and will be modelled by grouping items
//! rather than by changing [`DisplayItem`] itself.
//!
//! # Geometry source
//!
//! Items carry [`Rect`]s from this crate's geometry module for now. When
//! `jkva-layout` starts producing authoritative boxes, the geometry type is swapped
//! and these variants keep their shape, so the rest of the pipeline does not change.

use crate::geometry::{Color, Point, Rect};

/// Horizontal advance budgeted for one glyph when measuring a
/// [`DisplayItem::GlyphRun`], as a fraction of the em size.
///
/// [`DisplayItem::GlyphRun`] has to report bounds before `jkva-text` exists, and real
/// text metrics (advance widths, kerning, fallback) belong to that crate. Without them
/// the only safe assumption is the widest common case, so every character is budgeted a
/// full em — the advance of a full-width glyph in a CJK script, and comfortably above
/// the roughly `0.5 em` average of proportional Latin text.
///
/// The estimate is therefore deliberately generous: it over-covers, which costs a
/// little overdraw, instead of under-covering, which would leave stale pixels on the
/// screen because the damage would be too small. It is not a hard upper bound (kerning
/// can expand a pair and some display fonts exceed one em), which is one more reason the
/// real fix is measured advances rather than a better guess.
///
/// The constant is public so that callers can predict damage rectangles, and so that the
/// number is visible rather than buried. It will be removed — together with the
/// estimation in [`DisplayItem::bounds`] — once shaping provides measured metrics.
pub const GLYPH_ADVANCE_BUDGET_EM: f32 = 1.0;

/// Estimated distance from the baseline up to the top of the em box, as a fraction of
/// the em size.
///
/// [`DisplayItem::GlyphRun`] stores a baseline origin, while bounds need a top edge;
/// this ratio converts between the two until real font metrics are available. The
/// vertical estimate models one em box placed with a typical ascender, so the run
/// occupies `[baseline - 0.8 em, baseline + 0.2 em]`: one em tall. Taller typefaces can
/// exceed that by a fraction of an em, so — like the horizontal budget — this is a
/// placeholder for `jkva-text` metrics, not a specification.
pub const ESTIMATED_GLYPH_ASCENT_EM: f32 = 0.8;

/// One operation in a [`DisplayList`].
///
/// Variants are complete descriptions of a draw call: they carry everything the
/// recorder needs and reference nothing outside the list (an image is identified by a
/// source key, not by pixels). Colours are 8-bit RGBA in straight alpha, see
/// [`Color`].
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayItem {
    /// A solid axis-aligned rectangle.
    Rect {
        /// Area to fill, in layer space.
        rect: Rect,
        /// Fill colour, RGBA.
        color: Color,
    },

    /// A solid rectangle with rounded corners.
    RoundedRect {
        /// Area to fill, in layer space.
        rect: Rect,
        /// Corner radius in CSS pixels, clamped by the rasteriser to `min(w, h) / 2`.
        radius: f32,
        /// Fill colour, RGBA.
        color: Color,
    },

    /// A rectangular border painted *inside* `rect`.
    ///
    /// The border is inset (`box-sizing: border-box`), so it never paints outside
    /// `rect` and the bounds are exactly `rect`.
    Border {
        /// Outer edge of the border, in layer space.
        rect: Rect,
        /// Border thickness in CSS pixels, measured inwards from `rect`.
        width: f32,
        /// Border colour, RGBA.
        color: Color,
    },

    /// A run of text painted from a single baseline point.
    ///
    /// `origin` is the start of the baseline, and `size` is the em size (usually the
    /// CSS `font-size`). The run is one string so that the paint layer never splits
    /// words itself; line breaking belongs to `jkva-layout` and shaping to
    /// `jkva-text`.
    ///
    /// Because advance widths are not known here yet, [`DisplayItem::bounds`] estimates
    /// the width from the character count and [`GLYPH_ADVANCE_BUDGET_EM`]. The estimate
    /// is deliberately generous; see that constant for the reasoning.
    GlyphRun {
        /// Start of the baseline, in layer space.
        origin: Point,
        /// Text to paint. Whitespace and shaping are preserved verbatim.
        text: String,
        /// Em size in CSS pixels.
        size: f32,
        /// Text colour, RGBA.
        color: Color,
    },

    /// An image decoded elsewhere, identified by a source key.
    ///
    /// `source` is an opaque key into the image cache owned by a later layer of the
    /// pipeline (eventually the URL or the decoded-image handle); the paint layer
    /// stores it and never resolves it.
    Image {
        /// Destination rectangle, in layer space.
        rect: Rect,
        /// Opaque image source key.
        source: String,
    },
}

impl DisplayItem {
    /// The rectangle this item can paint into, in layer space.
    ///
    /// For filled and image items this is exactly the stored rectangle. For
    /// [`DisplayItem::GlyphRun`] the bounds are estimated (see the variant docs), and
    /// a run with a non-positive or NaN `size` has no bounds at all, because nothing
    /// sensible can be painted with an invalid font size.
    ///
    /// The bounds may be empty, for example for an empty `Rect` or an empty text run.
    /// Callers that accumulate bounds must therefore use [`Rect::union`], which treats
    /// empty rectangles as the identity.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Rect { rect, .. }
            | Self::RoundedRect { rect, .. }
            | Self::Border { rect, .. }
            | Self::Image { rect, .. } => *rect,
            Self::GlyphRun {
                origin, text, size, ..
            } => {
                if *size <= 0.0 || size.is_nan() {
                    return Rect::EMPTY;
                }
                let glyphs = text.chars().count() as f32;
                let width = glyphs * *size * GLYPH_ADVANCE_BUDGET_EM;
                let ascent = *size * ESTIMATED_GLYPH_ASCENT_EM;
                Rect::new(origin.x, origin.y - ascent, width, *size)
            }
        }
    }
}

/// An ordered list of [`DisplayItem`]s that describes one layer of a frame.
///
/// The list keeps paint order: index `0` is painted first. It owns its items, so it can
/// be retained across frames and diffed later; cloning it is a plain deep copy of the
/// item data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DisplayList {
    /// Paint-ordered items; item `0` is at the bottom, the last item is on top.
    pub items: Vec<DisplayItem>,
}

impl DisplayList {
    /// Creates an empty display list.
    #[must_use]
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Appends `item` on top of everything already recorded.
    pub fn push(&mut self, item: DisplayItem) {
        self.items.push(item);
    }

    /// Number of recorded items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` when nothing has been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Iterates over items from the bottom of the paint order to the top.
    pub fn iter(&self) -> std::slice::Iter<'_, DisplayItem> {
        self.items.iter()
    }

    /// The smallest rectangle covering every item, or [`Rect::EMPTY`] for an empty list.
    ///
    /// Empty items are ignored, so a list of degenerate items has empty bounds. The
    /// cost is proportional to the number of items plus the total text length, because
    /// glyph-run bounds are estimated on demand.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        self.items
            .iter()
            .map(DisplayItem::bounds)
            .fold(Rect::EMPTY, |acc, bounds| acc.union(&bounds))
    }

    /// Drops every item, keeping the allocated capacity.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

impl<'a> IntoIterator for &'a DisplayList {
    type Item = &'a DisplayItem;
    type IntoIter = std::slice::Iter<'a, DisplayItem>;

    /// Iterates in paint order, bottom first.
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Size;

    /// A red opaque fill used by several tests.
    fn red() -> Color {
        [255, 0, 0, 255]
    }

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

    #[test]
    fn new_list_is_empty_and_has_empty_bounds() {
        let list = DisplayList::new();
        assert_eq!(list, DisplayList::default());
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert_eq!(list.bounds(), Rect::EMPTY);
        assert_eq!(list.iter().count(), 0);
    }

    #[test]
    fn push_appends_in_paint_order() {
        let mut list = DisplayList::new();
        list.push(DisplayItem::Rect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            color: red(),
        });
        list.push(DisplayItem::RoundedRect {
            rect: Rect::new(1.0, 1.0, 5.0, 5.0),
            radius: 2.0,
            color: red(),
        });
        list.push(DisplayItem::Image {
            rect: Rect::new(2.0, 2.0, 3.0, 3.0),
            source: "logo.png".to_string(),
        });

        assert_eq!(list.len(), 3);
        // The first item stays at the bottom: order is insertion order, not sorted.
        assert!(matches!(list.items[0], DisplayItem::Rect { .. }));
        assert!(matches!(list.items[1], DisplayItem::RoundedRect { .. }));
        assert!(matches!(list.items[2], DisplayItem::Image { .. }));
        // Iteration exposes the same order.
        let ids: Vec<usize> = list
            .iter()
            .map(|item| match item {
                DisplayItem::Rect { .. } => 0,
                DisplayItem::RoundedRect { .. } => 1,
                DisplayItem::Image { .. } => 2,
                _ => 3,
            })
            .collect();
        assert_eq!(ids, vec![0, 1, 2]);
        assert_eq!((&list).into_iter().count(), 3);
    }

    #[test]
    fn bounds_are_the_union_of_item_bounds() {
        let mut list = DisplayList::new();
        list.push(DisplayItem::Rect {
            rect: Rect::new(10.0, 10.0, 20.0, 20.0),
            color: red(),
        });
        list.push(DisplayItem::Border {
            rect: Rect::new(50.0, 0.0, 10.0, 10.0),
            width: 2.0,
            color: red(),
        });
        assert_eq!(list.bounds(), Rect::new(10.0, 0.0, 50.0, 30.0));
    }

    #[test]
    fn border_bounds_are_the_border_box() {
        // The border is inset, so a 4 px border does not grow the bounds.
        let item = DisplayItem::Border {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            width: 4.0,
            color: red(),
        };
        assert_eq!(item.bounds(), Rect::new(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn image_bounds_are_the_destination_rect() {
        let item = DisplayItem::Image {
            rect: Rect::new(3.0, 4.0, 5.0, 6.0),
            source: "a.png".to_string(),
        };
        assert_eq!(item.bounds(), Rect::new(3.0, 4.0, 5.0, 6.0));
    }

    #[test]
    fn glyph_run_bounds_are_estimated_from_character_count() {
        let item = DisplayItem::GlyphRun {
            origin: Point::new(100.0, 200.0),
            text: "abcd".to_string(),
            size: 10.0,
            color: red(),
        };
        // 4 characters at one em each of a 10 px font, one em tall, on the baseline.
        assert_rect_close(item.bounds(), Rect::new(100.0, 192.0, 40.0, 10.0));
        // The budget is deliberately loose: proportional Latin text is normally well
        // under one em per character, so this can only over-cover.
        assert!(item.bounds().width() >= 4.0 * 10.0 * GLYPH_ADVANCE_BUDGET_EM);
    }

    #[test]
    fn glyph_run_uses_characters_not_bytes() {
        // "ě" is two UTF-8 bytes but one character, so a byte-based count would double
        // the estimated width.
        let czech = DisplayItem::GlyphRun {
            origin: Point::new(0.0, 10.0),
            text: "ě".to_string(),
            size: 10.0,
            color: red(),
        };
        assert_rect_close(czech.bounds(), Rect::new(0.0, 2.0, 10.0, 10.0));

        // A full-width CJK character is exactly one em, which is why one em is the
        // budget rather than the narrower Latin average.
        let cjk = DisplayItem::GlyphRun {
            origin: Point::new(0.0, 10.0),
            text: "漢字".to_string(),
            size: 10.0,
            color: red(),
        };
        assert_rect_close(cjk.bounds(), Rect::new(0.0, 2.0, 20.0, 10.0));
    }

    #[test]
    fn degenerate_glyph_runs_have_empty_bounds() {
        let empty_text = DisplayItem::GlyphRun {
            origin: Point::new(5.0, 5.0),
            text: String::new(),
            size: 12.0,
            color: red(),
        };
        assert!(empty_text.bounds().is_empty());

        let zero_size = DisplayItem::GlyphRun {
            origin: Point::new(5.0, 5.0),
            text: "hi".to_string(),
            size: 0.0,
            color: red(),
        };
        assert!(zero_size.bounds().is_empty());

        let negative_size = DisplayItem::GlyphRun {
            origin: Point::new(5.0, 5.0),
            text: "hi".to_string(),
            size: -12.0,
            color: red(),
        };
        assert!(negative_size.bounds().is_empty());

        let nan_size = DisplayItem::GlyphRun {
            origin: Point::new(5.0, 5.0),
            text: "hi".to_string(),
            size: f32::NAN,
            color: red(),
        };
        assert!(nan_size.bounds().is_empty());
    }

    #[test]
    fn empty_items_do_not_contribute_to_bounds() {
        let mut list = DisplayList::new();
        list.push(DisplayItem::Rect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            color: red(),
        });
        list.push(DisplayItem::Rect {
            rect: Rect::EMPTY,
            color: red(),
        });
        list.push(DisplayItem::GlyphRun {
            origin: Point::new(100.0, 100.0),
            text: String::new(),
            size: 10.0,
            color: red(),
        });
        assert_eq!(list.bounds(), Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn clear_empties_the_list() {
        let mut list = DisplayList::new();
        list.push(DisplayItem::Image {
            rect: Rect::new(0.0, 0.0, 4.0, 4.0),
            source: "x".to_string(),
        });
        list.clear();
        assert!(list.is_empty());
        assert_eq!(list.bounds(), Rect::EMPTY);
        // The list is usable again after clearing.
        list.push(DisplayItem::Rect {
            rect: Rect::new(1.0, 1.0, 2.0, 2.0),
            color: red(),
        });
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn items_compare_by_value() {
        // Display lists are retained and diffed, so value equality must be structural.
        let mut a = DisplayList::new();
        let mut b = DisplayList::new();
        a.push(DisplayItem::Rect {
            rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            color: red(),
        });
        b.push(DisplayItem::Rect {
            rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            color: red(),
        });
        assert_eq!(a, b);
        b.push(DisplayItem::Image {
            rect: Rect::from_origin_size(Point::ZERO, Size::new(1.0, 1.0)),
            source: "different".to_string(),
        });
        assert_ne!(a, b);
    }
}
