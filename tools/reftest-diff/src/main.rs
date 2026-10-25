//! CLI for reftest-diff. See `--help`.

use reftest_diff::{compare_rgba, decode_png, diff_rgba, write_png};
use std::path::PathBuf;
use std::process::ExitCode;

struct Args {
    expected: PathBuf,
    actual: PathBuf,
    threshold: u8,
    max_ratio: f64,
    out: Option<PathBuf>,
}

fn usage() -> ! {
    eprintln!(
        "usage: reftest-diff <expected.png> <actual.png> [--threshold N] [--max-ratio R] [--out diff.png]\n\
         \n\
         Exit codes: 0 = within tolerance, 1 = different, 2 = error."
    );
    std::process::exit(2);
}

fn parse() -> Args {
    let mut it = std::env::args().skip(1);
    let (Some(expected), Some(actual)) = (it.next(), it.next()) else {
        usage();
    };
    let mut args = Args {
        expected: expected.into(),
        actual: actual.into(),
        threshold: 0,
        max_ratio: 0.0,
        out: None,
    };
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--threshold" => {
                args.threshold = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--max-ratio" => {
                args.max_ratio = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--out" => args.out = Some(it.next().unwrap_or_else(|| usage()).into()),
            _ => usage(),
        }
    }
    args
}

fn main() -> ExitCode {
    let args = parse();
    let (expected, ew, eh) = match decode_png(&args.expected) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("reftest-diff: {e}");
            return ExitCode::from(2);
        }
    };
    let (actual, aw, ah) = match decode_png(&args.actual) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("reftest-diff: {e}");
            return ExitCode::from(2);
        }
    };
    if (ew, eh) != (aw, ah) {
        eprintln!("reftest-diff: size mismatch: {ew}x{eh} vs {aw}x{ah}");
        return ExitCode::from(1);
    }
    let report = match compare_rgba(&expected, &actual, ew, eh, args.threshold) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("reftest-diff: {e}");
            return ExitCode::from(2);
        }
    };
    if let Some(out) = &args.out {
        match diff_rgba(&expected, &actual, ew, eh, args.threshold)
            .and_then(|d| write_png(out, &d, ew, eh))
        {
            Ok(()) => println!("diff image: {}", out.display()),
            Err(e) => {
                eprintln!("reftest-diff: {e}");
                return ExitCode::from(2);
            }
        }
    }
    println!(
        "{}x{}: {}/{} pixels differ ({:.4}%), max delta {}, threshold {}",
        report.width,
        report.height,
        report.differing,
        report.total,
        report.ratio() * 100.0,
        report.max_delta,
        args.threshold
    );
    if report.ratio() > args.max_ratio {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
