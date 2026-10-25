//! CLI for bench-runner. `bench-runner [--iterations N] [--rows N] [--rules N] [--json]`

use std::time::{Duration, Instant};

use bench_runner::{css_fixture, html_fixture, median, total};
use jkva_dom::parse_document;
use jkva_style::{StyleEngine, Stylesheet};
use jkva_text::{FontCache, TextStyle};

struct Args {
    iterations: usize,
    rows: usize,
    rules: usize,
    json: bool,
}

fn usage() -> ! {
    eprintln!(
        "usage: bench-runner [--iterations N] [--rows N] [--rules N] [--json]\n\
         \n\
         Measures HTML parse, CSS parse, the cascade and text shaping on generated fixtures.\n\
         Iteration counts and fixture sizes are part of every result."
    );
    std::process::exit(2);
}

fn parse_args() -> Args {
    let mut args = Args {
        iterations: 20,
        rows: 200,
        rules: 200,
        json: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--iterations" => {
                args.iterations = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--rows" => {
                args.rows = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--rules" => {
                args.rules = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--json" => args.json = true,
            _ => usage(),
        }
    }
    args
}

fn measure(iterations: usize, mut body: impl FnMut()) -> Vec<Duration> {
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        body();
        samples.push(start.elapsed());
    }
    samples
}

fn main() {
    let args = parse_args();
    let html = html_fixture(args.rows);
    let css = css_fixture(args.rules);

    let samples = measure(args.iterations, || {
        let document = parse_document(&html);
        std::hint::black_box(document.root());
    });
    let parse_total = total(&samples);
    let parse_median = median(samples);

    let samples = measure(args.iterations, || {
        let sheet = Stylesheet::parse(&css).expect("fixture CSS parses");
        std::hint::black_box(sheet.rules().len());
    });
    let css_total = total(&samples);
    let css_median = median(samples);

    let document = parse_document(&html);
    let sheet = Stylesheet::parse(&css).expect("fixture CSS parses");
    let samples = measure(args.iterations, || {
        let mut engine = StyleEngine::new();
        engine.add_stylesheet(sheet.clone());
        let styles = engine.compute_styles(&document);
        std::hint::black_box(styles.len());
    });
    let cascade_total = total(&samples);
    let cascade_median = median(samples);

    let mut fonts = FontCache::new();
    let style = TextStyle {
        size: 16.0,
        ..TextStyle::default()
    };
    let samples = measure(args.iterations, || {
        let shaped = fonts.shape("Jak se máš, JKvo? Hello world.", &style);
        std::hint::black_box(shaped.width);
    });
    let shape_total = total(&samples);
    let shape_median = median(samples);

    let report = [
        ("parse-html", parse_total, parse_median),
        ("parse-css", css_total, css_median),
        ("cascade", cascade_total, cascade_median),
        ("shape-text", shape_total, shape_median),
    ];

    if args.json {
        println!("[");
        for (i, (name, total, median)) in report.iter().enumerate() {
            println!(
                "  {{\"name\":\"{name}\",\"fixture_rows\":{},\"fixture_rules\":{},\"iterations\":{},\"total_ms\":{:.3},\"median_us\":{:.1}}}{}",
                args.rows,
                args.rules,
                args.iterations,
                total.as_secs_f64() * 1000.0,
                median.as_secs_f64() * 1_000_000.0,
                if i + 1 == report.len() { "" } else { "," }
            );
        }
        println!("]");
    } else {
        println!(
            "{:<12} {:>8} {:>12} {:>12}",
            "bench", "iters", "total ms", "median µs"
        );
        for (name, total, median) in report {
            println!(
                "{:<12} {:>8} {:>12.2} {:>12.1}",
                name,
                args.iterations,
                total.as_secs_f64() * 1000.0,
                median.as_secs_f64() * 1_000_000.0
            );
        }
        println!(
            "\nfixture: {} rows / {} nodes, {} rules; HTML {} bytes, CSS {} bytes",
            args.rows,
            args.rows * 3 + 2,
            args.rules,
            html.len(),
            css.len()
        );
    }
}
