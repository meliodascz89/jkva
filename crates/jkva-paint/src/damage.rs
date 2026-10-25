//! Damage tracking: the set of areas a layer must repaint.
//!
//! A retained layer keeps its display list between frames (ADR-017). When something
//! changes, the engine does not repaint the whole layer; it repaints the areas the
//! change invalidated and presents only those. A [`DamageRegion`] accumulates those
//! areas.
//!
//! # The damage-merging algorithm
//!
//! A region is a *small set of rectangles*, not one bounding box and not a per-pixel
//! mask. A handful of rectangles is what makes partial repaint pay off: the compositor
//! scissors a few areas instead of re-recording the whole surface, so a blinking caret
//! costs a few square pixels instead of a full frame. An unbounded pile of tiny
//! rectangles is just as bad, though — each one costs a scissor rectangle, a draw call
//! and driver overhead — so [`DamageRegion::add`] coalesces rectangles that overlap or
//! merely sit close to each other.
//!
//! Adding one rectangle runs these steps:
//!
//! 1. **Reject empty rectangles.** A rectangle covering no pixels (zero or negative
//!    extent, or NaN — see [`Rect::is_empty`]) cannot invalidate anything and is
//!    dropped. This keeps degenerate layout output from inflating the region, and keeps
//!    [`Rect::EMPTY`] the identity of the union.
//! 2. **Merge with every rectangle it touches.** The new rectangle is the candidate.
//!    "Touches" means the two boxes overlap, *or* their separation on each axis is at
//!    most [`DamageRegion::merge_gap`]. On a touch the candidate becomes the bounding
//!    box of both (`Rect::union`) and the absorbed rectangle is removed.
//! 3. **Restart the scan after every merge, until a pass merges nothing.** Growing the
//!    candidate can put it close to a rectangle that was already inspected, so one pass
//!    is not enough; merging must run to a fixed point. That is what makes it
//!    transitive: if `A` touches `B` and `B` touches `C`, all three collapse into one
//!    rectangle even when `A` and `C` are far apart.
//! 4. **Append when nothing touched.** Otherwise the final candidate becomes a new
//!    region rectangle.
//!
//! # Why a small gap threshold
//!
//! Layout and text routinely produce damage that is *almost* adjacent: a 1 px seam
//! between two inline boxes, an underline one pixel below text, a caret beside the
//! character it follows. Treating those as separate regions doubles the scissor count
//! for no visual benefit, because repainting one extra pixel is cheaper than a second
//! draw call. [`DEFAULT_MERGE_GAP`] is therefore `8.0` CSS pixels: wide enough to
//! collapse seams and text-plus-caret, narrow enough to keep genuinely separate areas
//! (two distant paragraphs, a sidebar and the content beside it) as separate regions.
//! It is a measured-by-feel heuristic, not a specification constant; tune it with
//! [`DamageRegion::with_merge_gap`] once frame timings say otherwise.
//!
//! # Guarantees
//!
//! - **Coverage.** Every rectangle passed to [`DamageRegion::add`] (or contained in a
//!   region passed to [`DamageRegion::union`]) is contained in some rectangle of the
//!   resulting region until [`DamageRegion::clear`]. Repainting the region therefore
//!   always repaints everything that was destroyed; damage is never silently lost.
//! - **Pairwise separation.** After every `add`, no two rectangles of the region overlap
//!   or lie within the merge gap of each other, because a new rectangle is merged with
//!   all touching rectangles to a fixed point. Coalesced rectangles therefore never
//!   double-cover a pixel.
//! - **Conservative, not minimal.** Merging covers the gap between merged rectangles, so
//!   the region may cover pixels that did not change. That is overdraw, never
//!   underdraw: correctness first, minimality never.
//! - **Order dependent at the margins.** Because of the proximity threshold, adding the
//!   same rectangles in a different order can yield a different, still covering, set.
//!   Tests pin behaviour that matters (coverage, counts after obvious merges) rather
//!   than an exact rectangle order.
//!
//! # Cost
//!
//! `add` is `O(n²)` in the number of region rectangles `n`, because every merge restarts
//! the scan and removing from the middle of the vector shifts the tail. `n` is the
//! number of disjoint dirty areas in one frame — single digits to low tens in practice —
//! and appending is amortised `O(1)`. If a profile ever shows this dominating, replace
//! the scan with a spatial index; the public contract does not change.
//!
//! # Version 1 limits
//!
//! - No pixel mask, no per-pixel granularity, no damage rectangles smaller than the item
//!   that produced them (glyph bounds are estimated, see [`crate::display_list`]).
//! - No clipping to the layer bounds; the caller decides scissor bounds at present time.
//! - Rectangles are in layer space. Damage from transforms between layers, and damage
//!   produced by diffing two display lists, arrive with the layer cache in `jkva-gfx`.
//! - Rectangles are not sorted, so the paint layer makes no promise about draw order.

use crate::geometry::Rect;

/// Default proximity threshold used when merging damage rectangles, in CSS pixels.
///
/// Rectangles that are separated by at most this distance on both axes are merged into
/// their bounding box. See the [module documentation](self) for the rationale and for
/// how to tune it per region with [`DamageRegion::with_merge_gap`].
pub const DEFAULT_MERGE_GAP: f32 = 8.0;

/// A set of coalesced rectangles that must be repainted.
///
/// The set is built with [`DamageRegion::add`], which keeps it small by merging
/// rectangles that overlap or are close together; the merging algorithm and its
/// guarantees are documented at the [module level](self).
///
/// Damage regions are ordinary values: they can be cloned, kept per layer and threaded
/// through the frame loop. Iteration order is insertion order and carries no meaning;
/// presentation just scissorses each rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct DamageRegion {
    /// Coalesced rectangles, pairwise separated by more than the merge gap.
    rects: Vec<Rect>,
    /// Proximity threshold applied by [`DamageRegion::add`], in CSS pixels.
    merge_gap: f32,
}

impl DamageRegion {
    /// Creates an empty region using [`DEFAULT_MERGE_GAP`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rects: Vec::new(),
            merge_gap: DEFAULT_MERGE_GAP,
        }
    }

    /// Creates an empty region with a custom merge gap.
    ///
    /// The gap is the distance, in CSS pixels, below which two rectangles are merged
    /// into their bounding box. A negative or NaN gap is clamped to `0.0`, which means
    /// "merge only rectangles that overlap or share an edge". Increasing the gap
    /// produces fewer, larger rectangles: cheaper to present, more overdraw.
    #[must_use]
    pub fn with_merge_gap(gap: f32) -> Self {
        // `f32::max` returns the other operand when one is NaN, so this also turns NaN
        // into 0.0.
        let gap = gap.max(0.0);
        Self {
            rects: Vec::new(),
            merge_gap: gap,
        }
    }

    /// The merge gap this region applies, in CSS pixels.
    #[must_use]
    pub fn merge_gap(&self) -> f32 {
        self.merge_gap
    }

    /// Marks `rect` as damaged, merging it with the rectangles it touches.
    ///
    /// Empty rectangles are ignored. The merging algorithm — proximity threshold,
    /// transitive restart, and why the result is conservative rather than minimal — is
    /// documented on the [module](self).
    pub fn add(&mut self, rect: Rect) {
        if rect.is_empty() {
            return;
        }

        let mut merged = rect;
        let mut index = 0;
        while index < self.rects.len() {
            if touches(&merged, &self.rects[index], self.merge_gap) {
                merged = merged.union(&self.rects[index]);
                // Removing from the middle keeps the remaining rectangles in insertion
                // order so that results are reproducible for a given call sequence.
                self.rects.remove(index);
                // The candidate grew and may now reach a rectangle seen earlier in this
                // pass; rescan from the start until a full pass merges nothing.
                index = 0;
            } else {
                index += 1;
            }
        }

        self.rects.push(merged);
    }

    /// Absorbs every rectangle of `other` into this region.
    ///
    /// The rectangles are re-merged using *this* region's merge gap, so a region built
    /// with a different gap does not leak its threshold. Equivalent to calling
    /// [`DamageRegion::add`] for each rectangle of `other`.
    pub fn union(&mut self, other: &Self) {
        for rect in &other.rects {
            self.add(*rect);
        }
    }

    /// Returns `true` when nothing needs repainting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    /// Number of coalesced rectangles, not the number of calls to
    /// [`DamageRegion::add`]; merges reduce it.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rects.len()
    }

    /// The smallest rectangle covering all damaged area, or [`Rect::EMPTY`] when the
    /// region is empty.
    ///
    /// Useful for a fast full-surface test; prefer presenting the individual
    /// [`DamageRegion::rects`] so partial repaint stays partial.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        self.rects
            .iter()
            .fold(Rect::EMPTY, |acc, rect| acc.union(rect))
    }

    /// Drops all damaged area, keeping the allocated capacity and the merge gap.
    pub fn clear(&mut self) {
        self.rects.clear();
    }

    /// The coalesced rectangles, for the compositor to scissor against.
    ///
    /// The slice is pairwise separated by more than the merge gap, but not sorted and
    /// not promised to be minimal.
    #[must_use]
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// Iterates over the coalesced rectangles.
    pub fn iter(&self) -> std::slice::Iter<'_, Rect> {
        self.rects.iter()
    }
}

impl Default for DamageRegion {
    /// An empty region using [`DEFAULT_MERGE_GAP`].
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a DamageRegion {
    type Item = &'a Rect;
    type IntoIter = std::slice::Iter<'a, Rect>;

    fn into_iter(self) -> Self::IntoIter {
        self.rects.iter()
    }
}

/// Separating distance between two intervals, `0.0` when they overlap or touch.
fn axis_gap(a_min: f32, a_max: f32, b_min: f32, b_max: f32) -> f32 {
    (b_min - a_max).max(a_min - b_max).max(0.0)
}

/// Returns `true` when two non-empty rectangles overlap or are at most `gap` apart on
/// both axes.
fn touches(a: &Rect, b: &Rect, gap: f32) -> bool {
    let horizontal = axis_gap(a.min_x(), a.max_x(), b.min_x(), b.max_x());
    let vertical = axis_gap(a.min_y(), a.max_y(), b.min_y(), b.max_y());
    horizontal <= gap && vertical <= gap
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asserts that `rect` is covered by some rectangle of the region.
    fn assert_covered(region: &DamageRegion, rect: Rect) {
        assert!(
            region
                .rects()
                .iter()
                .any(|owned| owned.contains_rect(&rect)),
            "damage {rect:?} is not covered by {:?}",
            region.rects()
        );
    }

    #[test]
    fn new_region_is_empty() {
        let region = DamageRegion::new();
        assert!(region.is_empty());
        assert_eq!(region.len(), 0);
        assert_eq!(region.bounds(), Rect::EMPTY);
        assert_eq!(region.merge_gap(), DEFAULT_MERGE_GAP);
        assert_eq!(region, DamageRegion::default());
        assert_eq!(region.iter().count(), 0);
    }

    #[test]
    fn adding_a_rect_sets_bounds() {
        let mut region = DamageRegion::new();
        let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
        region.add(rect);
        assert!(!region.is_empty());
        assert_eq!(region.len(), 1);
        assert_eq!(region.bounds(), rect);
        assert_eq!(region.rects(), &[rect]);
    }

    #[test]
    fn empty_rects_are_ignored() {
        let mut region = DamageRegion::new();
        region.add(Rect::EMPTY);
        region.add(Rect::new(0.0, 0.0, 0.0, 10.0));
        region.add(Rect::new(0.0, 0.0, 10.0, 0.0));
        region.add(Rect::new(f32::NAN, 0.0, 10.0, 10.0));
        assert!(region.is_empty());
        assert_eq!(region.bounds(), Rect::EMPTY);
    }

    #[test]
    fn overlapping_rects_merge_into_one_region() {
        let mut region = DamageRegion::new();
        region.add(Rect::new(0.0, 0.0, 10.0, 10.0));
        region.add(Rect::new(5.0, 5.0, 10.0, 10.0));
        assert_eq!(region.len(), 1);
        assert_eq!(region.bounds(), Rect::new(0.0, 0.0, 15.0, 15.0));
    }

    #[test]
    fn nearby_rects_merge_within_the_gap() {
        let mut region = DamageRegion::with_merge_gap(2.0);
        region.add(Rect::new(0.0, 0.0, 10.0, 10.0));
        // Two pixels to the right: within the gap, so one bounding box.
        region.add(Rect::new(12.0, 0.0, 10.0, 10.0));
        assert_eq!(region.len(), 1);
        assert_eq!(region.bounds(), Rect::new(0.0, 0.0, 22.0, 10.0));
    }

    #[test]
    fn distant_rects_stay_separate() {
        let mut region = DamageRegion::with_merge_gap(2.0);
        let left = Rect::new(0.0, 0.0, 10.0, 10.0);
        let right = Rect::new(20.0, 0.0, 10.0, 10.0);
        region.add(left);
        region.add(right);
        assert_eq!(region.len(), 2);
        // Bounds still cover both, but the region itself is not inflated.
        assert_eq!(region.bounds(), Rect::new(0.0, 0.0, 30.0, 10.0));
        assert_covered(&region, left);
        assert_covered(&region, right);
    }

    #[test]
    fn zero_gap_merges_only_touching_or_overlapping_rects() {
        let mut region = DamageRegion::with_merge_gap(0.0);
        region.add(Rect::new(0.0, 0.0, 10.0, 10.0));
        // Sharing an edge counts as touching at gap 0.
        region.add(Rect::new(10.0, 0.0, 10.0, 10.0));
        assert_eq!(region.len(), 1);

        // A one-pixel gap is not touching at gap 0.
        let mut separated = DamageRegion::with_merge_gap(0.0);
        separated.add(Rect::new(0.0, 0.0, 10.0, 10.0));
        separated.add(Rect::new(11.0, 0.0, 10.0, 10.0));
        assert_eq!(separated.len(), 2);
    }

    #[test]
    fn negative_and_nan_gaps_are_clamped_to_zero() {
        assert_eq!(DamageRegion::with_merge_gap(-5.0).merge_gap(), 0.0);
        assert_eq!(DamageRegion::with_merge_gap(f32::NAN).merge_gap(), 0.0);
        assert_eq!(DamageRegion::with_merge_gap(3.5).merge_gap(), 3.5);
    }

    #[test]
    fn merging_is_transitive() {
        // A touches B, B touches C, but A and C are far apart: the fixed-point rescan
        // must collapse all three into one bounding box.
        let mut region = DamageRegion::with_merge_gap(1.0);
        region.add(Rect::new(0.0, 0.0, 10.0, 10.0)); // A
        region.add(Rect::new(20.0, 0.0, 10.0, 10.0)); // C, far from A
        assert_eq!(region.len(), 2);
        region.add(Rect::new(11.0, 0.0, 10.0, 10.0)); // B, bridges A and C
        assert_eq!(region.len(), 1);
        assert_eq!(region.bounds(), Rect::new(0.0, 0.0, 30.0, 10.0));
    }

    #[test]
    fn merging_is_transitive_across_both_axes() {
        // A diagonal chain: each rect touches the previous one, none touch A.
        let mut region = DamageRegion::with_merge_gap(0.0);
        region.add(Rect::new(0.0, 0.0, 10.0, 10.0));
        region.add(Rect::new(10.0, 10.0, 10.0, 10.0));
        region.add(Rect::new(20.0, 20.0, 10.0, 10.0));
        assert_eq!(region.len(), 1);
        assert_eq!(region.bounds(), Rect::new(0.0, 0.0, 30.0, 30.0));
    }

    #[test]
    fn bounds_of_several_regions_is_their_union() {
        let mut region = DamageRegion::with_merge_gap(0.0);
        region.add(Rect::new(0.0, 0.0, 10.0, 10.0));
        region.add(Rect::new(100.0, 50.0, 10.0, 10.0));
        region.add(Rect::new(-20.0, 30.0, 5.0, 5.0));
        assert_eq!(region.len(), 3);
        assert_eq!(region.bounds(), Rect::new(-20.0, 0.0, 130.0, 60.0));
    }

    #[test]
    fn clear_resets_the_region() {
        let mut region = DamageRegion::new();
        region.add(Rect::new(0.0, 0.0, 10.0, 10.0));
        region.clear();
        assert!(region.is_empty());
        assert_eq!(region.len(), 0);
        assert_eq!(region.bounds(), Rect::EMPTY);
        // Still usable afterwards.
        region.add(Rect::new(1.0, 1.0, 2.0, 2.0));
        assert_eq!(region.len(), 1);
    }

    #[test]
    fn union_absorbs_another_region() {
        let mut left = DamageRegion::with_merge_gap(4.0);
        left.add(Rect::new(0.0, 0.0, 10.0, 10.0));

        let mut right = DamageRegion::with_merge_gap(0.0);
        right.add(Rect::new(12.0, 0.0, 10.0, 10.0));

        left.union(&right);
        // The two-pixel gap is bridged by `left`'s gap, even though `right` would not
        // have merged them itself.
        assert_eq!(left.len(), 1);
        assert_eq!(left.bounds(), Rect::new(0.0, 0.0, 22.0, 10.0));
        // The absorbed region is left untouched.
        assert_eq!(right.len(), 1);
    }

    #[test]
    fn union_of_empty_regions_is_a_no_op() {
        let mut region = DamageRegion::new();
        let rect = Rect::new(3.0, 4.0, 5.0, 6.0);
        region.add(rect);
        region.union(&DamageRegion::new());
        assert_eq!(region.len(), 1);
        assert_eq!(region.bounds(), rect);
    }

    #[test]
    fn every_added_rect_stays_covered() {
        // The core safety property: repainting the region repaints all damage.
        let mut region = DamageRegion::with_merge_gap(3.0);
        let added = [
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Rect::new(5.0, 5.0, 10.0, 10.0),
            Rect::new(40.0, 0.0, 4.0, 4.0),
            Rect::new(43.0, 0.0, 4.0, 4.0),
            Rect::new(0.0, 40.0, 100.0, 2.0),
            Rect::new(200.0, 200.0, 1.0, 1.0),
        ];
        for rect in added {
            region.add(rect);
        }
        for rect in added {
            assert_covered(&region, rect);
        }
        // And the overall bounds cover every added rect too.
        let bounds = region.bounds();
        for rect in added {
            assert!(bounds.contains_rect(&rect));
        }
    }

    #[test]
    fn merged_regions_are_pairwise_separated() {
        let mut region = DamageRegion::with_merge_gap(2.0);
        for rect in [
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Rect::new(12.0, 0.0, 10.0, 10.0), // bridges into the first
            Rect::new(100.0, 100.0, 5.0, 5.0),
            Rect::new(0.0, 100.0, 5.0, 5.0),
        ] {
            region.add(rect);
        }
        let rects = region.rects();
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                // No pair may overlap or sit within the gap: that is exactly the
                // postcondition `add` re-establishes.
                assert!(
                    !touches(a, b, region.merge_gap()),
                    "{a:?} still touches {b:?}"
                );
            }
        }
    }

    #[test]
    fn iteration_matches_the_stored_rectangles() {
        let mut region = DamageRegion::with_merge_gap(0.0);
        region.add(Rect::new(0.0, 0.0, 1.0, 1.0));
        region.add(Rect::new(50.0, 50.0, 1.0, 1.0));
        let iterated: Vec<Rect> = region.iter().copied().collect();
        assert_eq!(iterated, region.rects());
        assert_eq!((&region).into_iter().count(), 2);
    }

    #[test]
    fn regions_compare_by_value() {
        let mut a = DamageRegion::new();
        let mut b = DamageRegion::new();
        assert_eq!(a, b);
        a.add(Rect::new(0.0, 0.0, 1.0, 1.0));
        assert_ne!(a, b);
        b.add(Rect::new(0.0, 0.0, 1.0, 1.0));
        assert_eq!(a, b);
    }
}
