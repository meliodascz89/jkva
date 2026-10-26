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

use jkva_gfx::{Rect, Scene, SceneItem, SceneRenderer};
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
    let mut game = false;
    let mut sprites = 200usize;
    let mut mode = "incremental".to_string();
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
            "--game" => game = true,
            "--sprites" => {
                sprites = it
                    .next()
                    .ok_or("--sprites needs a number")?
                    .parse()
                    .map_err(|_| "invalid --sprites")?
            }
            "--mode" => {
                mode = it.next().ok_or("--mode needs a value")?;
                if mode != "incremental" && mode != "full" {
                    return Err("--mode must be incremental or full".into());
                }
            }
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
    if game {
        return run_game(width, height, sprites, frames, warmup, &mode, json);
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

fn run_game(
    width: u32,
    height: u32,
    sprites: usize,
    frames: usize,
    warmup: usize,
    mode: &str,
    json: bool,
) -> Result<(), String> {
    if sprites == 0 {
        return Err("--sprites must be positive".into());
    }
    let mut renderer = SceneRenderer::new()?;
    let size = 16.0f32;
    let moving = (sprites / 20).max(1);
    let static_count = sprites - moving;
    let columns = (width as f32 / (size * 2.0)).max(1.0) as usize;
    let position = |index: usize, frame: usize| -> Rect {
        let row = index / columns;
        let column = index % columns;
        let mut x = 8.0 + column as f32 * size * 2.0;
        let y = 8.0 + row as f32 * size * 2.0;
        if index >= static_count {
            let phase = (index - static_count) as f32 * 0.37;
            x += ((frame as f32 * 0.05 + phase).sin() * 12.0).abs();
        }
        Rect::new(x, y, size, size)
    };
    let mut previous: Vec<Rect> = (0..sprites).map(|i| position(i, 0)).collect();
    let mut samples = Vec::with_capacity(frames);
    let mut drawn_total = 0usize;
    for frame in 0..(warmup + frames) {
        let mut scene = Scene::new();
        let mut damage: Vec<Rect> = Vec::new();
        if mode == "full" {
            for index in 0..sprites {
                scene.push(SceneItem::Rect {
                    rect: position(index, frame),
                    color: [40, 90, 200, 255],
                });
            }
        } else {
            if frame == 0 {
                for index in 0..static_count {
                    scene.push(SceneItem::Rect {
                        rect: position(index, frame),
                        color: [200, 200, 200, 255],
                    });
                }
            }
            for (index, old) in previous.iter_mut().enumerate().skip(static_count) {
                let new = position(index, frame);
                scene.push(SceneItem::Rect {
                    rect: *old,
                    color: [255, 255, 255, 255],
                });
                scene.push(SceneItem::Rect {
                    rect: new,
                    color: [220, 60, 60, 255],
                });
                damage.push(*old);
                damage.push(new);
                *old = new;
            }
        }
        let start = Instant::now();
        if mode == "full" {
            renderer.submit(width, height, &scene)?;
        } else {
            renderer.submit_incremental(width, height, &scene, &damage)?;
        }
        let elapsed = start.elapsed();
        drawn_total += renderer.last_drawn_items();
        if frame >= warmup {
            samples.push(elapsed);
        }
    }
    let stats = stats(samples);
    let average_drawn = drawn_total as f64 / (frames + warmup) as f64;
    if json {
        println!(
            "{{\"mode\":\"{mode}\",\"sprites\":{sprites},\"moving\":{moving},\"width\":{width},\"height\":{height},\"frames\":{frames},\"p50_ms\":{:.3},\"p95_ms\":{:.3},\"avg_drawn_items\":{average_drawn:.2}}}",
            stats.p50.as_secs_f64() * 1000.0,
            stats.p95.as_secs_f64() * 1000.0
        );
    } else {
        println!(
            "game {mode}: {sprites} sprites ({moving} moving) at {width}x{height}, {frames} frames (+{warmup} warm-up)"
        );
        println!(
            "frame p50 {:.3} ms  p95 {:.3} ms  avg drawn items {average_drawn:.1}",
            stats.p50.as_secs_f64() * 1000.0,
            stats.p95.as_secs_f64() * 1000.0
        );
    }
    Ok(())
}
