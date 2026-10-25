//! CLI for reftest-runner. `reftest-runner <corpus-dir> [--update] [--width N] [--height N]`

use std::path::PathBuf;
use std::process::ExitCode;

struct Args {
    dir: PathBuf,
    update: bool,
    width: u32,
    height: u32,
}

fn usage() -> ! {
    eprintln!(
        "usage: reftest-runner <corpus-dir> [--update] [--width N] [--height N]\n\
         \n\
         Renders every <dir>/*.html with jkva-shot and compares it to <dir>/expected/<name>.png.\n\
         Set JKVA_SHOT to point at the jkva-shot binary (default: target/debug/jkva-shot).\n\
         Exit codes: 0 = all match, 1 = differences or missing images, 2 = error."
    );
    std::process::exit(2);
}

fn parse() -> Args {
    let mut it = std::env::args().skip(1);
    let Some(dir) = it.next() else { usage() };
    let mut args = Args {
        dir: dir.into(),
        update: false,
        width: 800,
        height: 600,
    };
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--update" => args.update = true,
            "--width" => {
                args.width = it.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage());
            }
            "--height" => {
                args.height = it.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage());
            }
            _ => usage(),
        }
    }
    args
}

fn shot_binary() -> PathBuf {
    std::env::var_os("JKVA_SHOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/debug/jkva-shot"))
}

fn main() -> ExitCode {
    let args = parse();
    let cases = match reftest_runner::discover(&args.dir) {
        Ok(cases) => cases,
        Err(e) => {
            eprintln!("reftest-runner: {e}");
            return ExitCode::from(2);
        }
    };
    if cases.is_empty() {
        eprintln!("reftest-runner: no *.html pages in {}", args.dir.display());
        return ExitCode::from(2);
    }
    let shot = shot_binary();
    let actual_dir = std::env::temp_dir().join("jkva-reftest-actual");
    let _ = std::fs::create_dir_all(&actual_dir);
    let _ = std::fs::create_dir_all(args.dir.join("expected"));

    let mut failures = 0;
    for case in &cases {
        let actual = actual_dir.join(format!("{}.png", case.name));
        let status = std::process::Command::new(&shot)
            .arg(&case.html)
            .arg(&actual)
            .arg(args.width.to_string())
            .arg(args.height.to_string())
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                println!("FAIL {} (renderer exited with {s})", case.name);
                failures += 1;
                continue;
            }
            Err(e) => {
                eprintln!("reftest-runner: cannot run {}: {e}", shot.display());
                return ExitCode::from(2);
            }
        }

        if args.update {
            if let Err(e) = std::fs::copy(&actual, &case.expected) {
                eprintln!("reftest-runner: cannot update {}: {e}", case.expected.display());
                return ExitCode::from(2);
            }
            println!("UPDATE {}", case.name);
            continue;
        }
        if !case.expected.exists() {
            println!("FAIL {} (missing {})", case.name, case.expected.display());
            failures += 1;
            continue;
        }
        match reftest_runner::compare(&case.expected, &actual, 0) {
            Ok(0.0) => println!("ok   {}", case.name),
            Ok(ratio) => {
                println!("FAIL {} ({:.4}% pixels differ)", case.name, ratio * 100.0);
                failures += 1;
            }
            Err(e) => {
                println!("FAIL {} ({e})", case.name);
                failures += 1;
            }
        }
    }
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
