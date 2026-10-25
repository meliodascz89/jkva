//! CLI for wpt-report; run it without arguments to print usage to stderr.

use std::process::ExitCode;
use wpt_report::summarize;

fn usage() -> ! {
    eprintln!(
        "usage: wpt-report <report.json> [--min-pass-rate R] [--top N]\n\
         \n\
         Exit codes: 0 = pass rate reached, 1 = below target, 2 = error."
    );
    std::process::exit(2);
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else { usage() };
    let mut min_rate = 0.0f64;
    let mut top = 12usize;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--min-pass-rate" => {
                min_rate = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--top" => {
                top = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            _ => usage(),
        }
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("wpt-report: cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let summary = match summarize(&text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("wpt-report: {e}");
            return ExitCode::from(2);
        }
    };

    println!(
        "{:<14} {:>8} {:>8} {:>8} {:>9}",
        "directory", "passed", "failed", "skipped", "pass rate"
    );
    let mut dirs: Vec<_> = summary.by_dir.iter().collect();
    dirs.sort_by(|a, b| b.1.failed.cmp(&a.1.failed).then(a.0.cmp(b.0)));
    for (name, c) in dirs.into_iter().take(top) {
        println!(
            "{:<14} {:>8} {:>8} {:>8} {:>8.2}%",
            name,
            c.passed,
            c.failed,
            c.skipped,
            c.pass_rate() * 100.0
        );
    }
    println!(
        "{:<14} {:>8} {:>8} {:>8} {:>8.2}%",
        "TOTAL",
        summary.overall.passed,
        summary.overall.failed,
        summary.overall.skipped,
        summary.pass_rate() * 100.0
    );

    if summary.pass_rate() < min_rate {
        eprintln!(
            "wpt-report: pass rate {:.2}% is below the target {:.2}%",
            summary.pass_rate() * 100.0,
            min_rate * 100.0
        );
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
