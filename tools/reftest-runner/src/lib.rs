//! Screenshot corpus runner for JKva.
//!
//! A corpus directory contains `*.html` pages and an `expected/` directory with
//! one PNG per page. The runner renders each page with `jkva-shot`, compares the
//! result with the expected image through `reftest-diff`, and can refresh the
//! expected images with `--update`.

#![deny(missing_docs)]

use std::path::{Path, PathBuf};

/// One corpus case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// File stem, used for the expected image name.
    pub name: String,
    /// The HTML source.
    pub html: PathBuf,
    /// The expected PNG.
    pub expected: PathBuf,
}

/// Finds every `*.html` page in `dir`, sorted by name.
///
/// # Errors
///
/// Returns an error when the directory cannot be read.
pub fn discover(dir: &Path) -> Result<Vec<Case>, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut cases = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read an entry: {e}"))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("html") {
            continue;
        }
        let Some(stem) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        cases.push(Case {
            name: stem.clone(),
            html: path,
            expected: dir.join("expected").join(format!("{stem}.png")),
        });
    }
    cases.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(cases)
}

/// Compares two PNG files and returns the differing-pixel ratio.
///
/// A missing actual file or a size mismatch is an error for the caller to report
/// as a failure; the ratio is `0.0` only when the images are identical.
///
/// # Errors
///
/// Returns an error when either image cannot be decoded or the sizes differ.
pub fn compare(expected: &Path, actual: &Path, threshold: u8) -> Result<f64, String> {
    let (expected_px, ew, eh) = reftest_diff::decode_png(expected)?;
    let (actual_px, aw, ah) = reftest_diff::decode_png(actual)?;
    if (ew, eh) != (aw, ah) {
        return Err(format!("size mismatch: {ew}x{eh} vs {aw}x{ah}"));
    }
    let report = reftest_diff::compare_rgba(&expected_px, &actual_px, ew, eh, threshold)?;
    Ok(report.ratio())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_html_files_and_maps_expected_names() {
        let dir = std::env::temp_dir().join(format!("jkva-reftest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("expected")).unwrap();
        std::fs::write(dir.join("b.html"), "<p>b</p>").unwrap();
        std::fs::write(dir.join("a.html"), "<p>a</p>").unwrap();
        std::fs::write(dir.join("notes.txt"), "ignore").unwrap();

        let cases = discover(&dir).unwrap();
        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0].name, "a");
        assert_eq!(cases[1].name, "b");
        assert_eq!(cases[0].expected, dir.join("expected/a.png"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_directory_is_an_error() {
        assert!(discover(Path::new("/nonexistent/jkva/corpus")).is_err());
    }
}
