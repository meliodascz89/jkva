//! Frame pacing for games and `requestAnimationFrame` (ADR-026).
//!
//! A browser that only renders on demand cannot pace a game: the loop must know
//! the display cadence, how many fixed simulation steps are due, and how far the
//! render is between two steps so it can interpolate. [`FrameClock`] owns that
//! arithmetic and nothing else — no threads, no timers, no platform calls — so a
//! test drives it with explicit instants and a game loop can drive it from
//! `CADisplayLink` or `requestAnimationFrame` later.
//!
//! The clock is monotonic: instants are [`Duration`]s measured from any shared
//! epoch and must never go backwards. A backwards instant is treated as "no time
//! passed", not as a panic.

#![allow(clippy::cast_precision_loss)]

use std::time::Duration;

/// Default cap on simulation steps in one frame.
///
/// A long stall (debugger, window drag, sleeping laptop) must not run hundreds of
/// physics steps at once; the cap keeps the loop responsive and is the standard
/// "spiral of death" guard.
pub const DEFAULT_MAX_STEPS: u32 = 5;

/// What a game loop should do for one rendered frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameStep {
    /// Monotonic index of this frame, starting at zero.
    pub frame: u64,
    /// Fixed simulation steps to run before rendering.
    pub steps: u32,
    /// Interpolation factor between the previous and the next simulation step,
    /// in `[0.0, 1.0]`.
    pub alpha: f32,
    /// Deadline of the next frame, measured from the same epoch.
    pub next_deadline: Duration,
    /// Frames that were skipped because the caller arrived late.
    pub dropped: u64,
}

/// Fixed-timestep frame clock.
#[derive(Clone, Debug)]
pub struct FrameClock {
    period: Duration,
    next_deadline: Duration,
    previous_deadline: Duration,
    frame: u64,
    max_steps: u32,
    origin: Duration,
}

impl FrameClock {
    /// Creates a clock at `hz` frames per second, starting at instant zero.
    ///
    /// `hz` is clamped to `1..=1000`; a display cannot meaningfully ask for more,
    /// and a zero period would divide by zero.
    #[must_use]
    pub fn new(hz: u32) -> Self {
        Self::with_start(hz, Duration::ZERO)
    }

    /// Creates a clock whose first deadline is `start`.
    #[must_use]
    pub fn with_start(hz: u32, start: Duration) -> Self {
        let hz = hz.clamp(1, 1000);
        let period = Duration::from_secs_f64(1.0 / f64::from(hz));
        Self {
            period,
            next_deadline: start + period,
            previous_deadline: start,
            frame: 0,
            max_steps: DEFAULT_MAX_STEPS,
            origin: start,
        }
    }

    /// Overrides the cap on simulation steps per frame.
    #[must_use]
    pub fn with_max_steps(mut self, max_steps: u32) -> Self {
        self.max_steps = max_steps.max(1);
        self
    }

    /// Changes the cadence, keeping the current deadline phase.
    ///
    /// Used when a window moves to a display with another refresh rate.
    pub fn set_rate(&mut self, hz: u32) {
        let hz = hz.clamp(1, 1000);
        self.period = Duration::from_secs_f64(1.0 / f64::from(hz));
    }

    /// Frames per second of the current period.
    #[must_use]
    pub fn rate(&self) -> f64 {
        1.0 / self.period.as_secs_f64()
    }

    /// Duration of one frame.
    #[must_use]
    pub fn period(&self) -> Duration {
        self.period
    }

    /// Index of the next frame to render.
    #[must_use]
    pub fn frame_index(&self) -> u64 {
        self.frame
    }

    /// Advances to the frame that contains `now`.
    ///
    /// Returns the simulation steps that became due since the previous call (capped
    /// at [`FrameClock::with_max_steps`]), the interpolation alpha, and how many
    /// frames were skipped. Calling it with an instant before the current deadline
    /// is idempotent: the same frame index and `steps == 0` come back.
    pub fn advance(&mut self, now: Duration) -> FrameStep {
        let now = now.max(self.previous_deadline);
        let elapsed = now.saturating_sub(self.previous_deadline);
        let raw_steps = if self.period.is_zero() {
            0
        } else {
            (elapsed.as_nanos() / self.period.as_nanos()) as u64
        };
        let dropped = raw_steps.saturating_sub(u64::from(self.max_steps));
        let steps = raw_steps.min(u64::from(self.max_steps)) as u32;

        // Move the deadline forward by the steps actually simulated, so alpha always
        // describes the fraction of the *next* step that has already elapsed.
        let advanced = self.period * steps;
        self.previous_deadline += advanced;
        self.next_deadline = self.previous_deadline + self.period;
        self.frame += 1;

        let alpha = if self.period.is_zero() {
            0.0
        } else {
            (now.saturating_sub(self.previous_deadline).as_secs_f64() / self.period.as_secs_f64())
                .clamp(0.0, 1.0) as f32
        };
        FrameStep {
            frame: self.frame - 1,
            steps,
            alpha,
            next_deadline: self.next_deadline,
            dropped,
        }
    }

    /// Instant the clock started from.
    #[must_use]
    pub fn origin(&self) -> Duration {
        self.origin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn one_frame_at_sixty_hertz_advances_one_step() {
        let mut clock = FrameClock::new(60);
        let step = clock.advance(clock.period());
        assert_eq!(step.steps, 1);
        assert_eq!(step.frame, 0);
        assert!(step.alpha.abs() < 1.0e-6);
        assert_eq!(step.dropped, 0);
    }

    #[test]
    fn a_late_frame_reports_dropped_frames_and_caps_steps() {
        let mut clock = FrameClock::new(60).with_max_steps(2);
        // 5 frames of time passed at once.
        let step = clock.advance(clock.period() * 5);
        assert_eq!(step.steps, 2);
        assert_eq!(step.dropped, 3);
    }

    #[test]
    fn alpha_interpolates_between_steps() {
        let mut clock = FrameClock::new(10);
        let step = clock.advance(clock.period() + clock.period() / 2);
        assert_eq!(step.steps, 1);
        assert!((step.alpha - 0.5).abs() < 0.01, "alpha {}", step.alpha);
    }

    #[test]
    fn an_early_call_is_idempotent() {
        let mut clock = FrameClock::new(60);
        let first = clock.advance(clock.period());
        let second = clock.advance(clock.period() / 2);
        assert_eq!(second.steps, 0);
        assert_eq!(second.frame, first.frame + 1);
    }

    #[test]
    fn a_rate_change_keeps_the_clock_usable() {
        let mut clock = FrameClock::new(60);
        clock.set_rate(120);
        assert!((clock.rate() - 120.0).abs() < 0.5);
        let step = clock.advance(clock.period());
        assert_eq!(step.steps, 1);
    }

    #[test]
    fn zero_and_huge_rates_are_clamped() {
        assert!(FrameClock::new(0).period() >= Duration::from_millis(1));
        assert!(FrameClock::new(100_000).period() >= Duration::from_millis(1));
    }

    #[test]
    fn a_backwards_instant_does_not_panic() {
        let mut clock = FrameClock::with_start(60, ms(100));
        let step = clock.advance(ms(50));
        assert_eq!(step.steps, 0);
    }
}
