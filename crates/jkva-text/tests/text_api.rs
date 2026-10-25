//! Black-box tests of the public `jkva-text` API.
//!
//! They run against the real system fonts, because getting a face out of the
//! machine is exactly what this crate is for; a mocked font database would test
//! the mock. Every test names what it needs, so a machine with an unexpected
//! font set fails with an explanation instead of a bare assertion.

use jkva_text::{FontCache, TextStyle};

/// A cache over the machine's system fonts.
fn cache() -> FontCache {
    FontCache::new()
}

/// The default style is Helvetica 16 px, weight 400, upright (documented API).
#[test]
fn default_style_matches_the_documented_values() {
    let style = TextStyle::default();
    assert_eq!(style.family, "Helvetica");
    assert_eq!(style.size, 16.0);
    assert_eq!(style.weight, 400);
    assert!(!style.italic);
}

/// "Hello" is one glyph per letter, and the run has a measurable width.
#[test]
fn shapes_hello_into_five_glyphs() {
    let mut fonts = cache();
    let run = fonts.shape("Hello", &TextStyle::default());
    assert_eq!(run.glyphs.len(), 5, "one glyph per ASCII letter");
    assert!(run.width > 0.0, "a five-letter word has a positive width");
    for glyph in &run.glyphs {
        assert!(glyph.advance > 0.0, "glyph {} has no advance", glyph.id);
    }
}

/// An empty string shapes to nothing, but the line metrics survive.
#[test]
fn empty_text_is_a_zero_width_run() {
    let mut fonts = cache();
    let run = fonts.shape("", &TextStyle::default());
    assert!(run.glyphs.is_empty());
    assert_eq!(run.width, 0.0);
    assert!(
        run.ascent > 0.0 && run.descent > 0.0,
        "an empty run still needs a line box, got ascent {} descent {}",
        run.ascent,
        run.descent
    );
}

/// Both metrics are positive distances from the baseline.
#[test]
fn ascent_and_descent_are_positive() {
    let mut fonts = cache();
    let run = fonts.shape("Metrics", &TextStyle::default());
    assert!(run.ascent > 0.0, "ascent {} should be positive", run.ascent);
    assert!(
        run.descent > 0.0,
        "descent {} should be positive",
        run.descent
    );
    assert!(
        run.ascent > run.descent,
        "sane latin metrics have more space above the baseline: {} vs {}",
        run.ascent,
        run.descent
    );
}

/// A family that cannot exist falls back to a real face and says so.
#[test]
fn unknown_family_falls_back_with_a_warning() {
    let mut fonts = cache();
    let style = TextStyle::new("JKva Missing Family 4711", 16.0, 400, false);
    let run = fonts.shape("Fallback", &style);
    assert!(
        !run.glyphs.is_empty(),
        "the fallback face still shapes text"
    );
    assert!(run.width > 0.0);
    let warnings = fonts.warnings();
    assert!(!warnings.is_empty(), "a fallback must be recorded");
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("JKva Missing Family 4711")),
        "the warning must name the family that was missing: {warnings:?}"
    );
}

/// Shaping the same string twice reuses the cached face and agrees exactly.
#[test]
fn shaping_the_same_string_twice_is_deterministic() {
    let mut fonts = cache();
    let style = TextStyle::default();
    let first = fonts.shape("Cache me", &style);
    let second = fonts.shape("Cache me", &style);
    assert_eq!(first, second);
}

/// The reported width is exactly the sum of the advances it reports.
#[test]
fn width_is_the_sum_of_advances() {
    let mut fonts = cache();
    let run = fonts.shape("The quick brown fox", &TextStyle::default());
    let sum: f32 = run.glyphs.iter().map(|glyph| glyph.advance).sum();
    assert!(
        (run.width - sum).abs() < 1.0e-6,
        "width {} must equal the advance sum {}",
        run.width,
        sum
    );
}

/// Czech diacritics shape to real glyphs, not to `.notdef`.
#[test]
fn czech_text_shapes_without_missing_glyphs() {
    let mut fonts = cache();
    let run = fonts.shape("Příliš žluťoučký kůň", &TextStyle::default());
    assert!(
        run.glyphs.len() >= 15,
        "expected at least 15 glyphs for 19 letters, got {}",
        run.glyphs.len()
    );
    assert!(run.width > 0.0);
    assert!(
        run.glyphs.iter().all(|glyph| glyph.id != 0),
        "the selected face must cover Czech, but shaped `.notdef`: {run:?}"
    );
}

/// A letter rasterises to a bitmap with real coverage.
#[test]
fn rasterizing_a_glyph_produces_coverage() {
    let mut fonts = cache();
    let style = TextStyle::default();
    let run = fonts.shape("H", &style);
    let glyph = run.glyphs[0];
    let bitmap = fonts.rasterize(glyph.id, &style).expect("H must rasterise");
    assert!(bitmap.width > 0 && bitmap.height > 0, "{bitmap:?}");
    assert_eq!(
        bitmap.alpha.len(),
        (bitmap.width * bitmap.height) as usize,
        "one coverage byte per pixel"
    );
    let covered = bitmap.alpha.iter().filter(|&&alpha| alpha > 0).count();
    assert!(covered > 0, "glyph {} produced no coverage", glyph.id);
    let name = fonts
        .glyph_name(glyph.id, &style)
        .unwrap_or_else(|| format!("#{}", glyph.id));
    println!(
        "first rasterised glyph: id={} name={name} coverage={covered}/{} box={}x{} left={} top={}",
        glyph.id,
        bitmap.alpha.len(),
        bitmap.width,
        bitmap.height,
        bitmap.left,
        bitmap.top
    );
}

/// A glyph id the face does not have is refused, not drawn as garbage.
#[test]
fn unknown_glyph_ids_do_not_rasterize() {
    let mut fonts = cache();
    assert!(fonts.rasterize(u16::MAX, &TextStyle::default()).is_none());
}

/// Width scales linearly with `font-size`.
#[test]
fn doubling_the_size_doubles_the_width() {
    let mut fonts = cache();
    let small = fonts.shape("Scaling test", &TextStyle::default());
    let large = fonts.shape(
        "Scaling test",
        &TextStyle::new("Helvetica", 32.0, 400, false),
    );
    assert!(small.width > 0.0);
    let ratio = large.width / small.width;
    assert!((ratio - 2.0).abs() < 0.02, "16px to 32px ratio was {ratio}");
}

/// Every shaped glyph has a usable name for debugging.
#[test]
fn glyph_names_resolve_for_shaped_glyphs() {
    let mut fonts = cache();
    let style = TextStyle::default();
    let run = fonts.shape("A", &style);
    let name = fonts
        .glyph_name(run.glyphs[0].id, &style)
        .expect("a shaped glyph must have a name");
    assert!(!name.is_empty());
}

/// CSS generic families are understood as generic, not as literal family names.
#[test]
fn generic_family_keywords_resolve() {
    let mut fonts = cache();
    let run = fonts.shape(
        "sans-serif",
        &TextStyle::new("sans-serif", 16.0, 400, false),
    );
    assert!(!run.glyphs.is_empty());
    assert!(run.width > 0.0);
}
