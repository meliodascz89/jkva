//! CLI for gfx-bench:
//! `gfx-bench [--frames N] [--quads N] [--width N] [--height N] [--json]`
//!
//! The default run is 4096 quads at 1920x1080 for 120 frames. Run it in release
//! mode for numbers that mean anything: debug CPU submit time is dominated by
//! unoptimised encoding.

use std::process::ExitCode;

use gfx_bench::{BenchConfig, BenchReport, run};

struct Args {
    config: BenchConfig,
    json: bool,
}

fn usage() -> ! {
    eprintln!(
        "usage: gfx-bench [--frames N] [--quads N] [--width N] [--height N] [--json]\n\
         \n\
         Renders thousands of textured quads offscreen on the JKva Metal path and\n\
         reports CPU submit and GPU completion frame times. The scene size, target,\n\
         device and build are part of every report."
    );
    std::process::exit(2);
}

fn help() -> ! {
    println!(
        "gfx-bench — frame-time benchmark for the JKva Metal path\n\
         \n\
         usage: gfx-bench [--frames N] [--quads N] [--width N] [--height N] [--json]\n\
         \n\
         --frames N   measured frames (default 120, warm-up frames excluded)\n\
         --quads N    quads per frame, one draw call each (default 4096)\n\
         --width N    render target width in pixels (default 1920)\n\
         --height N   render target height in pixels (default 1080)\n\
         --json       print the full report as JSON\n\
         \n\
         Requires macOS with Metal. Run in release mode for comparable numbers."
    );
    std::process::exit(0);
}

fn parse_args() -> Args {
    let mut config = BenchConfig::default();
    let mut json = false;
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--frames" => {
                config.frames = it
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--quads" => {
                config.quads = it
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--width" => {
                config.width = it
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--height" => {
                config.height = it
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--json" => json = true,
            "-h" | "--help" => help(),
            _ => usage(),
        }
    }
    Args { config, json }
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn print_stats(label: &str, stats: &gfx_bench::FrameStats) {
    println!(
        "{label:<16} count={:<6} p50={:>8.3} ms  p95={:>8.3} ms  min={:>8.3} ms  max={:>8.3} ms",
        stats.count, stats.p50_ms, stats.p95_ms, stats.min_ms, stats.max_ms
    );
}

fn print_report(report: &BenchReport) {
    let scene = &report.scene;
    println!(
        "gfx-bench: {} quads/frame, {} draw calls, {}x{} target, {}x{} texture",
        scene.quads,
        scene.draw_calls_per_frame,
        scene.target_width,
        scene.target_height,
        scene.texture_width,
        scene.texture_height
    );
    println!(
        "device: {} (Apple7={} Apple8={} Apple9={} Metal4={} UMA={} timestamp-counters={})",
        report.device.name,
        report.device.apple7,
        report.device.apple8,
        report.device.apple9,
        report.device.metal4,
        report.device.unified_memory,
        report.device.timestamp_counters
    );
    println!(
        "budgets: max buffer {:.1} MiB, recommended working set {:.1} MiB",
        mib(report.device.max_buffer_length),
        mib(report.device.recommended_max_working_set_size)
    );
    println!(
        "frames: {} measured, {} warm-up; vertex buffer {} KiB; build {} ({})",
        report.config.frames,
        report.config.warmup_frames,
        report.scene.vertex_buffer_bytes / 1024,
        report.build.crate_version,
        report.build.profile
    );
    print_stats("cpu submit", &report.cpu_submit);
    print_stats("gpu completion", &report.gpu_completion);
    match (&report.gpu_execution, &report.gpu_execution_source) {
        (Some(stats), Some(source)) => {
            print_stats("gpu execution", stats);
            println!("gpu execution source: {source} (driver command-buffer timestamps)");
        }
        _ => println!("gpu execution: unavailable (driver timestamps not reported)"),
    }
    println!(
        "note: gpu completion is command-buffer wait wall time; it is not a GPU \
         timestamp query and includes queue/driver latency."
    );
}

fn main() -> ExitCode {
    let args = parse_args();
    let report = match run(&args.config) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("gfx-bench: {error}");
            return ExitCode::FAILURE;
        }
    };
    if args.json {
        match serde_json::to_string_pretty(&report) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("gfx-bench: could not serialise the report: {error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        print_report(&report);
    }
    ExitCode::SUCCESS
}
