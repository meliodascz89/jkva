//! `jkva-paint` — display lists, paint geometry and damage tracking for JKva.
//!
//! The paint layer sits between layout/text and `jkva-gfx` (see
//! `docs/src/en/architecture.md`). It turns the computed box tree into an ordered,
//! backend-agnostic description of what to draw, and it tracks which pixels a change
//! invalidated. Following ADR-017 the engine keeps retained layers, one display list per
//! layer and dirty regions, so an ordinary frame re-records and re-rasterises only what
//! actually changed instead of the whole surface.
//!
//! # What this crate is
//!
//! A plain data model plus the arithmetic around it. There is no GPU code, no
//! rasteriser, no font shaping, no image decoding and no dependency on `jkva-gfx`; a
//! display list has to be cheap to retain, easy to compare and testable without a
//! device.
//!
//! - [`geometry`] — [`Point`], [`Size`], [`Rect`] and the [`Color`] alias, in CSS pixels
//!   with the origin at the layer's top-left corner and `y` growing downwards.
//! - [`display_list`] — [`DisplayItem`] and [`DisplayList`]: an ordered list in which
//!   later items paint on top of earlier ones.
//! - [`damage`] — [`DamageRegion`], the coalesced set of areas that must be repainted,
//!   and the documented merging algorithm behind it.
//!
//! # What this crate is not (yet)
//!
//! Stacking contexts, clipping, transforms, shadows, gradients, filters, blend modes and
//! wide-gamut/HDR colour are all part of the ADR-017 target, but not part of this first
//! version. The display-list shape is deliberately simple so that those features arrive
//! as grouping and new item variants rather than as a rewrite.
//!
//! # Geometry will move
//!
//! Rectangles currently live in [`geometry`]. When `jkva-layout` starts producing
//! authoritative boxes, its type replaces the local one; sizes and parameters stay the
//! same, so [`DisplayItem`] and [`DisplayList`] do not change shape.
//!
//! # Example
//!
//! ```
//! use jkva_paint::{DamageRegion, DisplayItem, DisplayList, Point, Rect};
//!
//! // Build one layer: a white background with a black label on it.
//! let mut list = DisplayList::new();
//! list.push(DisplayItem::Rect {
//!     rect: Rect::new(0.0, 0.0, 100.0, 40.0),
//!     color: [255, 255, 255, 255],
//! });
//! list.push(DisplayItem::GlyphRun {
//!     origin: Point::new(8.0, 24.0),
//!     text: "JKva".to_string(),
//!     size: 16.0,
//!     color: [0, 0, 0, 255],
//! });
//! assert_eq!(list.len(), 2);
//! assert_eq!(list.items[0].bounds(), Rect::new(0.0, 0.0, 100.0, 40.0));
//! assert_eq!(list.bounds(), Rect::new(0.0, 0.0, 100.0, 40.0));
//!
//! // Two small repaints that are close together cost one region, not two.
//! let mut damage = DamageRegion::new();
//! damage.add(Rect::new(10.0, 10.0, 20.0, 20.0));
//! damage.add(Rect::new(32.0, 10.0, 20.0, 20.0));
//! assert_eq!(damage.len(), 1);
//! assert_eq!(damage.bounds(), Rect::new(10.0, 10.0, 42.0, 20.0));
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod damage;
pub mod display_list;
pub mod geometry;

pub use damage::{DEFAULT_MERGE_GAP, DamageRegion};
pub use display_list::{
    DisplayItem, DisplayList, ESTIMATED_GLYPH_ASCENT_EM, GLYPH_ADVANCE_BUDGET_EM,
};
pub use geometry::{Color, Point, Rect, Size};

/// Version of the JKva paint layer, taken from the crate manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_reported() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn reexports_are_usable_from_the_crate_root() {
        let rect = Rect::new(0.0, 0.0, 1.0, 1.0);
        let color: Color = [0, 0, 0, 255];
        let item = DisplayItem::Rect { rect, color };
        let mut list = DisplayList::new();
        list.push(item);
        let mut region = DamageRegion::with_merge_gap(DEFAULT_MERGE_GAP);
        region.add(list.bounds());
        assert_eq!(region.bounds(), rect);
    }
}
