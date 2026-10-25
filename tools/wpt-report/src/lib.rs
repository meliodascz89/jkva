//! Summarises a Web Platform Tests `--log-wptreport` JSON file.
//!
//! The runner produces one report per run; this crate turns it into pass rates,
//! overall and per top-level test directory, so a pull request can gate on a number
//! instead of a feeling.

#![deny(missing_docs)]

use serde_json::Value;
use std::collections::BTreeMap;

/// Passed, failed and skipped subtests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Subtests that passed.
    pub passed: u64,
    /// Subtests that failed, timed out, crashed or errored.
    pub failed: u64,
    /// Subtests that were skipped.
    pub skipped: u64,
}

impl Counts {
    /// Total subtests counted (including skipped).
    #[must_use]
    pub fn total(self) -> u64 {
        self.passed + self.failed + self.skipped
    }

    /// Pass rate over executed subtests, ignoring skipped ones.
    #[must_use]
    pub fn pass_rate(self) -> f64 {
        let executed = self.passed + self.failed;
        if executed == 0 {
            return 0.0;
        }
        self.passed as f64 / executed as f64
    }
}

/// A parsed report: overall counts plus counts per top-level directory.
#[derive(Clone, Debug, Default)]
pub struct Summary {
    /// Counts over the whole run.
    pub overall: Counts,
    /// Counts per top-level directory (`css`, `dom`, `html`, ...).
    pub by_dir: BTreeMap<String, Counts>,
}

impl Summary {
    /// Overall pass rate over executed subtests.
    #[must_use]
    pub fn pass_rate(&self) -> f64 {
        self.overall.pass_rate()
    }
}

/// Classifies a WPT status string into passed, failed or skipped.
fn classify(status: &str) -> Option<bool> {
    match status {
        "PASS" | "OK" => Some(true),
        "FAIL" | "ERROR" | "TIMEOUT" | "CRASH" | "PRECONDITION_FAILED" => Some(false),
        "SKIP" => None,
        _ => Some(false),
    }
}

/// Directory component of a WPT test path, or `"other"`.
fn directory(test: &str) -> String {
    test.trim_start_matches('/')
        .split('/')
        .next()
        .filter(|s| !s.is_empty() && !s.ends_with(".html"))
        .unwrap_or("other")
        .to_string()
}

/// Parses a wptreport JSON string into a [`Summary`].
///
/// # Errors
///
/// Returns an error when the JSON is invalid or has no `results` array.
pub fn summarize(json: &str) -> Result<Summary, String> {
    let root: Value =
        serde_json::from_str(json).map_err(|e| format!("invalid report JSON: {e}"))?;
    let results = root
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| "report has no `results` array".to_string())?;

    let mut summary = Summary::default();
    for result in results {
        let test = result
            .get("test")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let dir = directory(test);
        let add = |ok: Option<bool>, counts: &mut Counts| match ok {
            Some(true) => counts.passed += 1,
            Some(false) => counts.failed += 1,
            None => counts.skipped += 1,
        };

        let subtests = result.get("subtests").and_then(Value::as_array);
        match subtests {
            Some(subtests) if !subtests.is_empty() => {
                for sub in subtests {
                    let status = sub.get("status").and_then(Value::as_str).unwrap_or("FAIL");
                    let ok = classify(status);
                    add(ok, &mut summary.overall);
                    add(ok, summary.by_dir.entry(dir.clone()).or_default());
                }
            }
            _ => {
                let status = result
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("FAIL");
                let ok = classify(status);
                add(ok, &mut summary.overall);
                add(ok, summary.by_dir.entry(dir).or_default());
            }
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = r#"{
      "results": [
        {"test": "/css/one.html", "status": "OK", "subtests": [
          {"name": "a", "status": "PASS"},
          {"name": "b", "status": "FAIL"}
        ]},
        {"test": "/dom/two.html", "status": "PASS"},
        {"test": "/html/three.html", "status": "SKIP"}
      ]
    }"#;

    #[test]
    fn counts_subtests_and_top_level_statuses() {
        let s = summarize(REPORT).unwrap();
        assert_eq!(s.overall.passed, 2);
        assert_eq!(s.overall.failed, 1);
        assert_eq!(s.overall.skipped, 1);
        assert_eq!(s.overall.total(), 4);
        assert!((s.pass_rate() - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn groups_by_top_level_directory() {
        let s = summarize(REPORT).unwrap();
        assert_eq!(s.by_dir["css"].passed, 1);
        assert_eq!(s.by_dir["css"].failed, 1);
        assert_eq!(s.by_dir["dom"].passed, 1);
        assert_eq!(s.by_dir["html"].skipped, 1);
    }

    #[test]
    fn rejects_invalid_json_and_missing_results() {
        assert!(summarize("not json").is_err());
        assert!(summarize("{}").is_err());
    }

    #[test]
    fn empty_report_is_zero_not_nan() {
        let s = summarize(r#"{"results": []}"#).unwrap();
        assert_eq!(s.pass_rate(), 0.0);
    }
}
