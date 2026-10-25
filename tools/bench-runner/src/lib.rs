//! Deterministic micro-benchmarks for the engine layers.
//!
//! These are not a substitute for Speedometer or WPT: they measure one layer at a
//! time on a generated fixture so a regression is visible in seconds. Every report
//! names the fixture size and the iteration count, because a number without those
//! is not a measurement.

#![deny(missing_docs)]

use std::time::Duration;

/// Median of a non-empty sample.
///
/// # Panics
///
/// Panics when the sample is empty; callers always pass at least one iteration.
#[must_use]
pub fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

/// Total time of a sample.
#[must_use]
pub fn total(samples: &[Duration]) -> Duration {
    samples.iter().sum()
}

/// Generates an HTML fixture with `rows` divs, each containing a paragraph.
#[must_use]
pub fn html_fixture(rows: usize) -> String {
    let mut html = String::from("<!doctype html><html><body>");
    for i in 0..rows {
        html.push_str(&format!(
            "<div class=\"row row-{i}\"><p>Row {i} with some text to shape and lay out.</p></div>"
        ));
    }
    html.push_str("</body></html>");
    html
}

/// Generates a stylesheet with `rules` class rules and one paragraph rule.
#[must_use]
pub fn css_fixture(rules: usize) -> String {
    let mut css = String::from("p { color: #222222; margin: 0; }\n");
    for i in 0..rules {
        css.push_str(&format!(
            ".row-{i} {{ margin-top: {i}px; color: #123456; }}\n"
        ));
    }
    css
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_of_odd_and_even_samples() {
        let d = |ms| Duration::from_millis(ms);
        assert_eq!(median(vec![d(3), d(1), d(2)]), d(2));
        assert_eq!(median(vec![d(4), d(1), d(3), d(2)]), d(3));
    }

    #[test]
    fn fixtures_grow_with_their_parameter() {
        assert!(html_fixture(10).len() > html_fixture(1).len());
        assert!(css_fixture(10).len() > css_fixture(1).len());
        assert!(html_fixture(3).contains("row-2"));
        assert!(css_fixture(3).contains(".row-2"));
    }
}
