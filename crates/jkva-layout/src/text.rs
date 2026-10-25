//! Text measurement, laid-out text runs and the line-box constant.
//!
//! Layout does not shape text. It asks a [`TextMeasure`] for the advance of a string
//! and treats advances as additive, which is enough to break lines but is not
//! typography: shaping, kerning, ligatures, fallback fonts and bidirectional
//! reordering belong to `jkva-text` (ADR-015). The engine adapts its shaping output
//! to [`TextMeasure`] (`measure` becomes "shape and sum the advances"), and tests
//! substitute a fixed-advance mock.
//!
//! The dependency is inverted on purpose: `jkva-layout` must not depend on
//! `jkva-text`, because text would then drag a font stack (and `fontdb`, `harfrust`,
//! `swash`) into layout's dependency graph. [`TextMeasure`] is the seam.

use crate::geometry::Point;

/// The factor that turns a font size into a line box height, i.e. `line-height: 1.2`.
///
/// M1 has no `line-height` property and no font metrics. Using a constant multiple of
/// the font size keeps line stacking deterministic and measurable; a real line box
/// needs the font's ascent and descent and arrives with the text integration.
pub const LINE_HEIGHT_FACTOR: f32 = 1.2;

/// Measures the advance of a text run.
///
/// One call answers "how wide is this string at this font size, in CSS pixels". The
/// implementation may shape the string, cache the result or return a constant; layout
/// only requires that the answer is non-negative and finite for every input it passes
/// (a negative or non-finite result is clamped to `0.0` when it is used for line
/// breaking).
///
/// Layout calls `measure` repeatedly with prefixes, words and single spaces, and it
/// adds the results. An implementation whose advances are not additive (kerning
/// across word boundaries, for example) will be slightly off; the recorded
/// [`TextRun::width`] is the additive sum, not a re-measure of the whole fragment, so
/// layout and paint agree about where a run ends.
pub trait TextMeasure {
    /// Returns the advance of `text` at `font_size` CSS pixels, in CSS pixels.
    fn measure(&mut self, text: &str, font_size: f32) -> f32;

    /// Measures with the text style the cascade produced.
    ///
    /// The default ignores the style and delegates to [`TextMeasure::measure`], so
    /// simple implementations and test doubles keep working. The engine overrides
    /// it to shape with the real weight and family; layout and paint then agree
    /// about every advance.
    fn measure_styled(&mut self, text: &str, font_size: f32, weight: u16, family: &str) -> f32 {
        let _ = (weight, family);
        self.measure(text, font_size)
    }
}

/// One laid-out piece of a text node: a single line fragment.
///
/// An inline text node that fits on one line becomes exactly one [`TextRun`]; a text
/// node that wraps becomes one run per line, each in its own
/// [`LayoutBox`](crate::LayoutBox). Runs are ordered in paint order by their owning
/// box's position in the parent's `children`.
///
/// # Coordinates
///
/// [`origin`](Self::origin) is the top-left corner of the run's **line box**, not its
/// baseline: M1 has no font metrics, so the vertical placement of the glyphs inside
/// the line box (half-leading, baseline) is left to `jkva-text` and `jkva-paint`,
/// which have the shaped glyphs and their ascent/descent. The run's height is the
/// line box height, `LINE_HEIGHT_FACTOR * font_size`, owned by the box that contains
/// the run.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    /// The text of this fragment in document order, already whitespace-collapsed.
    ///
    /// A fragment that continues a line starts with one space, for example `" world"`
    /// when `hello world` was broken before `world`. Fragments never end with a space:
    /// trailing whitespace is dropped at the line break, as CSS `white-space: normal`
    /// requires.
    pub text: String,
    /// Top-left corner of the line box that holds this fragment, in document space.
    pub origin: Point,
    /// Font size in CSS pixels, copied from the style of the text node.
    pub font_size: f32,
    /// Text colour as straight sRGB RGBA, copied from the style of the text node.
    pub color: [u8; 4],
    /// Font weight used for measurement and painting.
    pub font_weight: u16,
    /// Font family used for measurement and painting; empty means the UA default.
    pub font_family: String,
    /// Advance of this fragment in CSS pixels: the additive sum used to place the next
    /// fragment and to decide the line break.
    pub width: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock that charges half an em per character.
    struct HalfEm;

    impl TextMeasure for HalfEm {
        fn measure(&mut self, text: &str, font_size: f32) -> f32 {
            text.chars().count() as f32 * font_size * 0.5
        }
    }

    #[test]
    fn line_height_factor_is_the_documented_1_2() {
        assert!((LINE_HEIGHT_FACTOR - 1.2).abs() < f32::EPSILON);
        // 16px text stacks in 19.2px line boxes.
        let line_height = LINE_HEIGHT_FACTOR * 16.0;
        assert!((line_height - 19.2).abs() < 1.0e-5);
    }

    #[test]
    fn measure_is_object_safe_and_usable_through_a_dyn_reference() {
        let mut measure = HalfEm;
        let dynamic: &mut dyn TextMeasure = &mut measure;
        assert_eq!(dynamic.measure("abc", 10.0), 15.0);
        assert_eq!(dynamic.measure("", 10.0), 0.0);
    }

    #[test]
    fn text_run_records_the_fields_it_is_given() {
        let run = TextRun {
            text: "hello".to_owned(),
            origin: Point::new(1.0, 2.0),
            font_size: 16.0,
            color: [0, 0, 0, 255],
            font_weight: 400,
            font_family: String::new(),
            width: 40.0,
        };
        assert_eq!(run.text, "hello");
        assert_eq!(run.origin, Point::new(1.0, 2.0));
        assert_eq!(run.font_size, 16.0);
        assert_eq!(run.width, 40.0);
        assert_eq!(run.clone(), run);
    }
}
