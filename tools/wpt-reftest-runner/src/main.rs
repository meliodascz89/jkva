//! CLI for wpt-reftest-runner.
//!
//! `wpt-reftest-runner <wpt-dir> [--limit N] [--filter S] [--threshold N] [--json]`

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use wpt_reftest_runner::{Case, discover};

struct Args {
    root: PathBuf,
    limit: Option<usize>,
    filter: Option<String>,
    threshold: f64,
    json: bool,
}

fn usage() -> ! {
    eprintln!(
        "usage: wpt-reftest-runner <wpt-dir> [--limit N] [--filter S] [--threshold N] [--json]\n\
         \n\
         Finds Web Platform Tests reftests (<link rel=match|mismatch>), renders both sides with\n\
         jkva-shot and compares them. Pages with external dependencies are reported as\n\
         unsupported, never as passing.\n\
         Set JKVA_SHOT to point at the renderer (default: target/debug/jkva-shot).\n\
         Exit codes: 0 = no failures, 1 = failures or errors, 2 = tool error."
    );
    std::process::exit(2);
}

fn parse() -> Args {
    let mut it = std::env::args().skip(1);
    let Some(root) = it.next() else { usage() };
    let mut args = Args {
        root: root.into(),
        limit: None,
        filter: None,
        threshold: 0.0,
        json: false,
    };
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--limit" => {
                args.limit = it.next().and_then(|v| v.parse().ok());
                if args.limit.is_none() {
                    usage();
                }
            }
            "--filter" => args.filter = it.next(),
            "--threshold" => {
                args.threshold = it.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage());
            }
            "--json" => args.json = true,
            _ => usage(),
        }
    }
    args
}

fn shot() -> PathBuf {
    std::env::var_os("JKVA_SHOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/debug/jkva-shot"))
}

fn render(shot: &Path, page: &Path, out: &Path) -> Result<(), String> {
    let status = std::process::Command::new(shot)
        .arg(page)
        .arg(out)
        .arg("800")
        .arg("600")
        .output()
        .map_err(|e| format!("cannot run {}: {e}", shot.display()))?;
    if !status.status.success() {
        return Err(String::from_utf8_lossy(&status.stderr).trim().to_string());
    }
    Ok(())
}

fn compare(expected: &Path, actual: &Path) -> Result<f64, String> {
    let (a, aw, ah) = reftest_diff::decode_png(expected)?;
    let (b, bw, bh) = reftest_diff::decode_png(actual)?;
    if (aw, ah) != (bw, bh) {
        return Err(format!("size mismatch {aw}x{ah} vs {bw}x{bh}"));
    }
    Ok(reftest_diff::compare_rgba(&a, &b, aw, ah, 0)?.ratio())
}

fn main() -> ExitCode {
    let args = parse();
    let mut cases = match discover(&args.root) {
        Ok(cases) => cases,
        Err(e) => {
            eprintln!("wpt-reftest-runner: {e}");
            return ExitCode::from(2);
        }
    };
    if let Some(filter) = &args.filter {
        cases.retain(|case| case.name.contains(filter));
    }
    if let Some(limit) = args.limit {
        cases.truncate(limit);
    }
    if cases.is_empty() {
        eprintln!("wpt-reftest-runner: no reftests found in {}", args.root.display());
        return ExitCode::from(2);
    }

    let shot = shot();
    let actual_dir = std::env::temp_dir().join("jkva-wpt-actual");
    let _ = std::fs::create_dir_all(&actual_dir);

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut unsupported = 0usize;
    let mut errors = 0usize;

    for (index, case) in cases.iter().enumerate() {
        if !case.unsupported.is_empty() {
            unsupported += 1;
            if !args.json {
                println!(
                    "skip  {} ({})",
                    case.name,
                    case.unsupported.join(", ")
                );
            }
            continue;
        }
        match run_case(case, index, &shot, &actual_dir, args.threshold) {
            Ok(true) => {
                passed += 1;
                if !args.json {
                    println!("pass  {}", case.name);
                }
            }
            Ok(false) => {
                failed += 1;
                if !args.json {
                    println!("FAIL  {}", case.name);
                }
            }
            Err(e) => {
                errors += 1;
                if !args.json {
                    println!("error {} ({e})", case.name);
                }
            }
        }
    }

    if args.json {
        println!(
            "{{\"total\":{},\"pass\":{passed},\"fail\":{failed},\"unsupported\":{unsupported},\"error\":{errors}}}",
            cases.len()
        );
    } else {
        println!(
            "\n{} reftests: {passed} pass, {failed} fail, {unsupported} unsupported, {errors} error",
            cases.len()
        );
    }

    if failed > 0 || errors > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn run_case(
    case: &Case,
    index: usize,
    shot: &Path,
    dir: &Path,
    threshold: f64,
) -> Result<bool, String> {
    let test_png = dir.join(format!("{index}-test.png"));
    let ref_png = dir.join(format!("{index}-ref.png"));
    render(shot, &case.test, &test_png)?;
    render(shot, &case.reference, &ref_png)?;
    let ratio = compare(&test_png, &ref_png)?;
    if case.mismatch {
        Ok(ratio > threshold)
    } else {
        Ok(ratio <= threshold)
    }
}
