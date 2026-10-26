//! End-to-end engine frame benchmark.
//!
//! Loads a page once (parse, cascade, layout), then measures the frame path that a
//! game or an animation loop repeats: build the scene (backgrounds, borders, glyph
//! shaping and rasterization), upload it and wait for the GPU. The CPU readback is
//! skipped, exactly as a presented frame would skip it.
//!
//! This is deliberately narrower than a browser benchmark: layout and style are
//! static here, so the number answers "what does one paint+submit frame cost",
//! not "what does a full navigation cost".

use std::time::{Duration, Instant};

use jkva_gfx::SceneRenderer;
use jkva_shell::Frame;

struct Stats {
    p50: Duration,
    p95: Duration,
    min: Duration,
    max: Duration,
}

fn stats(mut samples: Vec<Duration>) -> Stats {
    samples.sort_unstable();
    let at = |q: f64| -> Duration {
        let index = ((samples.len() - 1) as f64 * q).round() as usize;
        samples[index]
    };
    Stats {
        p50: at(0.50),
        p95: at(0.95),
        min: samples[0],
        max: samples[samples.len() - 1],
    }
}

fn main() -> Result<(), String> {
    let mut page = "examples/cards.html".to_string();
    let mut frames = 120usize;
    let mut width = 800u32;
    let mut height = 600u32;
    let mut warmup = 5usize;
    let mut json = false;
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--page" => page = it.next().ok_or("--page needs a path")?,
            "--frames" => {
                frames = it
                    .next()
                    .ok_or("--frames needs a number")?
                    .parse()
                    .map_err(|_| "invalid --frames")?
            }
            "--width" => {
                width = it
                    .next()
                    .ok_or("--width needs a number")?
                    .parse()
                    .map_err(|_| "invalid --width")?
            }
            "--height" => {
                height = it
                    .next()
                    .ok_or("--height needs a number")?
                    .parse()
                    .map_err(|_| "invalid --height")?
            }
            "--warmup" => {
                warmup = it
                    .next()
                    .ok_or("--warmup needs a number")?
                    .parse()
                    .map_err(|_| "invalid --warmup")?
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!(
                    "usage: engine-bench [--page PATH] [--frames N] [--width N] [--height N] [--warmup N] [--json]"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if frames == 0 {
        return Err("--frames must be positive".into());
    }
    let html = std::fs::read_to_string(&page).map_err(|e| format!("cannot read {page}: {e}"))?;
    let mut frame = Frame::new(&html, width, height)?;
    let mut renderer = SceneRenderer::new()?;

    for _ in 0..warmup {
        frame.submit_with(&mut renderer)?;
    }
    let mut samples = Vec::with_capacity(frames);
    for _ in 0..frames {
        let start = Instant::now();
        frame.submit_with(&mut renderer)?;
        samples.push(start.elapsed());
    }
    let stats = stats(samples);
    if json {
        println!(
            "{{\"page\":\"{page}\",\"width\":{width},\"height\":{height},\"frames\":{frames},\"p50_ms\":{:.3},\"p95_ms\":{:.3},\"min_ms\":{:.3},\"max_ms\":{:.3}}}",
            stats.p50.as_secs_f64() * 1000.0,
            stats.p95.as_secs_f64() * 1000.0,
            stats.min.as_secs_f64() * 1000.0,
            stats.max.as_secs_f64() * 1000.0
        );
    } else {
        println!("page {page} at {width}x{height}, {frames} frames (+{warmup} warm-up)");
        println!(
            "paint+submit p50 {:.3} ms  p95 {:.3} ms  min {:.3} ms  max {:.3} ms",
            stats.p50.as_secs_f64() * 1000.0,
            stats.p95.as_secs_f64() * 1000.0,
            stats.min.as_secs_f64() * 1000.0,
            stats.max.as_secs_f64() * 1000.0
        );
    }
    Ok(())
}
