//! Frame-time benchmark for the JKva Metal path.
//!
//! Gaming performance is a first-class 0.1 goal (ADR-026, `docs/src/en/gaming.md`).
//! This tool renders a synthetic stress scene offscreen — thousands of textured
//! quads, one shared vertex buffer and one shared texture — and measures two
//! things per frame:
//!
//! - **CPU submit time**: creating the command buffer, encoding all draws and
//!   calling `commit()`.
//! - **GPU completion time**: the wall-clock time from after `commit()` until
//!   `waitUntilCompleted()` returns. This is the command-buffer wait the task
//!   calls out; it includes queue and driver latency, not just GPU execution.
//!
//! When Metal exposes driver command-buffer timestamps, the report also carries
//! `GPUStartTime`/`GPUEndTime` deltas. Those are *not* `MTLCounterSampleBuffer`
//! timestamp queries: they timestamp one command buffer, not individual passes,
//! and they are absent on some devices. The report keeps them in a separate
//! column so the difference stays visible.
//!
//! The scene is deliberately the current engine's shape: one draw call per
//! display-list item, all items sharing one vertex buffer and one texture. That
//! measures per-item submit cost as well as fill rate. It is not a batched
//! renderer and it is not a full game frame: no layout, style, JS, WASM,
//! compositing, window presentation or input latency is in the number.

#![deny(missing_docs)]
// Metal is a C API; this tool is an unsafe boundary like `jkva-gfx` itself.
#![allow(unsafe_code)]

use serde::Serialize;

#[cfg(target_os = "macos")]
mod render;

/// Side length of the single texture every quad samples.
pub const TEXTURE_SIZE: u32 = 256;

/// Benchmark configuration.
///
/// Defaults are a 1080p target with 4096 quads for 120 frames, which matches
/// the resolution used by the ADR-026 frame-time target. Counts and dimensions
/// are part of every report.
#[derive(Clone, Debug, Serialize)]
pub struct BenchConfig {
    /// Number of measured frames. Warm-up frames are extra and are not reported.
    pub frames: u32,
    /// Number of quads in the scene, one draw call per quad per frame.
    pub quads: u32,
    /// Render-target width in pixels.
    pub width: u32,
    /// Render-target height in pixels.
    pub height: u32,
    /// Frames rendered and discarded before measurement (shader/pipeline warm-up).
    pub warmup_frames: u32,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            frames: 120,
            quads: 4096,
            width: 1920,
            height: 1080,
            warmup_frames: 3,
        }
    }
}

impl BenchConfig {
    fn validate(&self) -> Result<(), String> {
        if self.frames == 0 {
            return Err("--frames must be at least 1".into());
        }
        if self.frames > 1_000_000 {
            return Err("--frames must be at most 1,000,000".into());
        }
        if self.quads == 0 {
            return Err("--quads must be at least 1".into());
        }
        if self.width == 0 || self.height == 0 {
            return Err("render target dimensions must be non-zero".into());
        }
        Ok(())
    }
}

/// One measured frame.
#[derive(Clone, Debug, Serialize)]
pub struct FrameSample {
    /// Zero-based frame index within the measured run.
    pub frame: u32,
    /// CPU time for command-buffer creation, draw encoding and `commit()`.
    pub cpu_submit_ms: f64,
    /// Wall time from after `commit()` to the end of `waitUntilCompleted()`.
    pub gpu_completion_ms: f64,
    /// Driver `GPUStartTime`..`GPUEndTime` for this command buffer, when both
    /// timestamps are non-zero.
    pub gpu_execution_ms: Option<f64>,
}

/// Summary of one timing series, in milliseconds.
///
/// Percentiles use the nearest-rank method: for `n` samples the p-th
/// percentile is the `ceil(p / 100 * n)`-th smallest value (1-based, clamped to
/// the sample count). With very small `n` (for example 2 in the smoke test) the
/// percentile is therefore an actual measurement, not an interpolation.
#[derive(Clone, Debug, Serialize)]
pub struct FrameStats {
    /// Number of samples in the series.
    pub count: usize,
    /// Nearest-rank 50th percentile.
    pub p50_ms: f64,
    /// Nearest-rank 95th percentile.
    pub p95_ms: f64,
    /// Smallest sample.
    pub min_ms: f64,
    /// Largest sample.
    pub max_ms: f64,
}

impl FrameStats {
    fn from_samples(mut samples: Vec<f64>) -> Self {
        debug_assert!(!samples.is_empty());
        samples.sort_by(f64::total_cmp);
        Self {
            count: samples.len(),
            p50_ms: percentile(&samples, 50.0),
            p95_ms: percentile(&samples, 95.0),
            min_ms: samples[0],
            max_ms: samples[samples.len() - 1],
        }
    }
}

fn percentile(sorted: &[f64], percent: f64) -> f64 {
    debug_assert!(!sorted.is_empty());
    let rank = (percent / 100.0 * sorted.len() as f64).ceil() as usize;
    let index = rank.clamp(1, sorted.len()) - 1;
    sorted[index]
}

/// Identity of the benchmark binary, because a number without the build is not
/// a measurement.
#[derive(Clone, Debug, Serialize)]
pub struct BuildInfo {
    /// `CARGO_PKG_VERSION` of this tool.
    pub crate_version: String,
    /// `debug` or `release`; CPU submit time differs substantially between them.
    pub profile: String,
}

/// Hardware identity and budgets attached to a report.
#[derive(Clone, Debug, Serialize)]
pub struct DeviceInfo {
    /// Marketing name reported by Metal.
    pub name: String,
    /// Largest single Metal buffer in bytes.
    pub max_buffer_length: u64,
    /// macOS recommendation for resident GPU memory in bytes.
    pub recommended_max_working_set_size: u64,
    /// `MTLGPUFamilyApple7` support.
    pub apple7: bool,
    /// `MTLGPUFamilyApple8` support.
    pub apple8: bool,
    /// `MTLGPUFamilyApple9` support.
    pub apple9: bool,
    /// `MTLGPUFamilyMetal4` support.
    pub metal4: bool,
    /// Shared CPU/GPU memory (UMA).
    pub unified_memory: bool,
    /// Timestamp counter set plus stage-boundary sampling support.
    pub timestamp_counters: bool,
}

impl DeviceInfo {
    fn from_capabilities(capabilities: &jkva_gfx::GpuCapabilities) -> Self {
        Self {
            name: capabilities.device_name.clone(),
            max_buffer_length: capabilities.max_buffer_length,
            recommended_max_working_set_size: capabilities.recommended_max_working_set_size,
            apple7: capabilities.families.apple7,
            apple8: capabilities.families.apple8,
            apple9: capabilities.families.apple9,
            metal4: capabilities.families.metal4,
            unified_memory: capabilities.supports_unified_memory,
            timestamp_counters: capabilities.supports_timestamp_counters,
        }
    }
}

/// Size and shape of the rendered scene, so the number can be reproduced.
#[derive(Clone, Debug, Serialize)]
pub struct SceneInfo {
    /// Quads per frame.
    pub quads: u32,
    /// Draw calls per frame (one per quad in the current engine path).
    pub draw_calls_per_frame: u32,
    /// Vertices per frame (6 per quad, two triangles).
    pub vertices_per_frame: usize,
    /// Bytes of the shared vertex buffer.
    pub vertex_buffer_bytes: usize,
    /// Width of the shared texture in pixels.
    pub texture_width: u32,
    /// Height of the shared texture in pixels.
    pub texture_height: u32,
    /// Render-target width in pixels.
    pub target_width: u32,
    /// Render-target height in pixels.
    pub target_height: u32,
}

/// Result of one benchmark run.
#[derive(Clone, Debug, Serialize)]
pub struct BenchReport {
    /// Build identity.
    pub build: BuildInfo,
    /// Hardware identity and budgets.
    pub device: DeviceInfo,
    /// Scene shape.
    pub scene: SceneInfo,
    /// Requested configuration, including warm-up count.
    pub config: BenchConfig,
    /// Every measured frame, in order.
    pub samples: Vec<FrameSample>,
    /// CPU submit-time distribution.
    pub cpu_submit: FrameStats,
    /// GPU completion-time distribution (command-buffer wait).
    pub gpu_completion: FrameStats,
    /// Driver command-buffer execution-time distribution, when available.
    pub gpu_execution: Option<FrameStats>,
    /// Where `gpu_execution` came from, or `None` when it is unavailable.
    pub gpu_execution_source: Option<String>,
}

/// Runs the benchmark and returns the report.
///
/// # Errors
///
/// Returns an error when the configuration is invalid, Metal is unavailable, a
/// Metal object cannot be created, the scene exceeds `maxBufferLength`, or the
/// GPU reports a command-buffer failure.
pub fn run(config: &BenchConfig) -> Result<BenchReport, String> {
    config.validate()?;
    #[cfg(target_os = "macos")]
    {
        render::run(config)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = config;
        Err("gfx-bench needs macOS and Metal; this build has no GPU backend".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_uses_nearest_rank() {
        let samples = (1..=20).map(f64::from).collect::<Vec<_>>();
        assert_eq!(percentile(&samples, 50.0), 10.0);
        assert_eq!(percentile(&samples, 95.0), 19.0);
        assert_eq!(percentile(&samples, 100.0), 20.0);
        assert_eq!(percentile(&samples, 1.0), 1.0);
    }

    #[test]
    fn stats_report_count_extremes_and_percentiles() {
        let stats = FrameStats::from_samples(vec![4.0, 1.0, 3.0, 2.0]);
        assert_eq!(stats.count, 4);
        assert_eq!(stats.min_ms, 1.0);
        assert_eq!(stats.max_ms, 4.0);
        assert_eq!(stats.p50_ms, 2.0);
        assert_eq!(stats.p95_ms, 4.0);
    }

    #[test]
    fn zero_frames_is_rejected_before_touching_the_gpu() {
        let config = BenchConfig {
            frames: 0,
            ..BenchConfig::default()
        };
        assert!(run(&config).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn smoke_run_reports_two_frames() {
        let config = BenchConfig {
            frames: 2,
            quads: 64,
            width: 320,
            height: 180,
            warmup_frames: 0,
        };
        let report = match run(&config) {
            Ok(report) => report,
            Err(error) => {
                // GitHub's macOS runners can boot without a usable GPU; skip
                // there and assert on a real Apple Silicon machine.
                eprintln!("skipped: Metal unavailable: {error}");
                return;
            }
        };
        assert_eq!(report.config.frames, 2);
        assert_eq!(report.samples.len(), 2);
        assert_eq!(report.cpu_submit.count, 2);
        assert_eq!(report.gpu_completion.count, 2);
        assert_eq!(report.scene.quads, 64);
        assert!(!report.device.name.is_empty());
        assert!(report.samples.iter().all(|s| s.cpu_submit_ms >= 0.0));
        assert!(report.samples.iter().all(|s| s.gpu_completion_ms >= 0.0));
    }
}
