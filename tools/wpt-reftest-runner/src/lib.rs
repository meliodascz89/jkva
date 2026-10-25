//! Web Platform Tests reftest discovery and classification.
//!
//! A WPT reftest compares two pages pixel by pixel: the test and a reference
//! named by `<link rel="match" href="...">` (or `mismatch`). This crate finds
//! those pairs and decides whether the engine can honestly run them.
//!
//! A page that depends on an external stylesheet, a script, an image or an
//! iframe is **unsupported**, not passing: both pages would lose the same
//! resources and compare equal for the wrong reason. Reporting such a pair as a
//! pass would be a lie, which is why unsupported is a separate status.

#![deny(missing_docs)]

use std::path::{Path, PathBuf};

use jkva_dom::{Document, NodeKind, parse_document};

/// Why a reftest cannot be run yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsupported {
    /// Human-readable reasons, one per dependency.
    pub reasons: Vec<String>,
}

/// One discovered reftest pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// Display name, relative to the corpus root.
    pub name: String,
    /// The test page.
    pub test: PathBuf,
    /// The reference page named by `rel=match`/`rel=mismatch`.
    pub reference: PathBuf,
    /// `true` when the pages are required to differ.
    pub mismatch: bool,
    /// Reasons the engine cannot run this pair yet, empty when it can.
    pub unsupported: Vec<String>,
}

/// Finds every reftest pair below `root`, sorted by name.
///
/// # Errors
///
/// Returns an error when the directory cannot be read.
pub fn discover(root: &Path) -> Result<Vec<Case>, String> {
    let mut files = Vec::new();
    collect_html(root, &mut files)?;
    let mut cases = Vec::new();
    for path in files {
        if let Some(case) = classify(root, &path) {
            cases.push(case);
        }
    }
    cases.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(cases)
}

fn collect_html(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read an entry: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_html(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("html") {
            out.push(path);
        }
    }
    Ok(())
}

/// Parses one page and returns a case when it is a reftest.
fn classify(root: &Path, path: &Path) -> Option<Case> {
    let html = std::fs::read_to_string(path).ok()?;
    let document = parse_document(&html);
    let mut reference = None;
    let mut mismatch = false;
    let mut unsupported = Vec::new();

    for node in document.descendants(document.root()) {
        let NodeKind::Element(element) = document.kind(node) else {
            continue;
        };
        match element.name() {
            "link" => {
                let rel = document.get_attribute(node, "rel").unwrap_or_default();
                let rel: Vec<&str> = rel.split_whitespace().collect();
                if rel.contains(&"match") || rel.contains(&"mismatch") {
                    if let Some(href) = document.get_attribute(node, "href") {
                        reference = Some(resolve(path, href));
                        mismatch = rel.contains(&"mismatch");
                    }
                } else if rel.contains(&"stylesheet") {
                    unsupported.push("external stylesheet".to_string());
                }
            }
            "script" => unsupported.push("script".to_string()),
            "img" => unsupported.push("image".to_string()),
            "iframe" => unsupported.push("iframe".to_string()),
            "video" | "audio" => unsupported.push("media".to_string()),
            "object" | "embed" => unsupported.push("embedded object".to_string()),
            "style" => {
                let css = document.text_content(node);
                if css.contains("@import") {
                    unsupported.push("@import".to_string());
                }
                if css.contains("url(") {
                    unsupported.push("url() in a stylesheet".to_string());
                }
            }
            _ => {}
        }
    }

    let reference = reference?;
    let name = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    Some(Case {
        name,
        test: path.to_path_buf(),
        reference,
        mismatch,
        unsupported,
    })
}

fn resolve(page: &Path, href: &str) -> PathBuf {
    let href = href.split(['?', '#']).next().unwrap_or(href);
    match page.parent() {
        Some(dir) => dir.join(href),
        None => PathBuf::from(href),
    }
}

/// Returns the document's unsupported reasons when a page is not self-contained.
#[must_use]
pub fn unsupported_reasons(document: &Document) -> Vec<String> {
    let mut reasons = Vec::new();
    for node in document.descendants(document.root()) {
        if let NodeKind::Element(element) = document.kind(node) {
            match element.name() {
                "script" => reasons.push("script".to_string()),
                "img" => reasons.push("image".to_string()),
                _ => {}
            }
        }
    }
    reasons
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jkva-wpt-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn discovers_a_self_contained_pair() {
        let dir = temp_dir("pair");
        std::fs::write(
            dir.join("test.html"),
            r#"<link rel="match" href="ref.html"><style>div{background-color:red}</style><div></div>"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("ref.html"),
            "<style>div{background-color:red}</style><div></div>",
        )
        .unwrap();
        let cases = discover(&dir).unwrap();
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].reference, dir.join("ref.html"));
        assert!(cases[0].unsupported.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn external_dependencies_are_unsupported_not_passing() {
        let dir = temp_dir("deps");
        std::fs::write(
            dir.join("test.html"),
            r#"<link rel="match" href="ref.html"><link rel="stylesheet" href="a.css"><script>x()</script>"#,
        )
        .unwrap();
        std::fs::write(dir.join("ref.html"), "<p>ref</p>").unwrap();
        let cases = discover(&dir).unwrap();
        assert_eq!(cases.len(), 1);
        assert!(
            cases[0]
                .unsupported
                .iter()
                .any(|r| r == "external stylesheet")
        );
        assert!(cases[0].unsupported.iter().any(|r| r == "script"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pages_without_a_match_link_are_skipped() {
        let dir = temp_dir("skip");
        std::fs::write(dir.join("plain.html"), "<p>no reference</p>").unwrap();
        assert!(discover(&dir).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn mismatch_is_recorded() {
        let dir = temp_dir("mismatch");
        std::fs::write(
            dir.join("test.html"),
            r#"<link rel="mismatch" href="ref.html"><p>a</p>"#,
        )
        .unwrap();
        std::fs::write(dir.join("ref.html"), "<p>b</p>").unwrap();
        let cases = discover(&dir).unwrap();
        assert!(cases[0].mismatch);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
