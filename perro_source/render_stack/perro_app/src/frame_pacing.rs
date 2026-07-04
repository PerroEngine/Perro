//! Frame pacing: single source of truth for the frame-rate cap, CPU frame
//! deadlines, and the cached monitor refresh rate.
//!
//! The winit runner drives all pacing through this module, so cap
//! normalization, wake headroom, and vsync interaction live in exactly one
//! place. Project knobs stay two: `graphics.vsync` and
//! `runtime.frame_rate_cap` in project.toml.

use perro_runtime_api::sub_apis::FrameRateCap as RuntimeFrameRateCap;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
use winit::window::Window;

pub(crate) const MIN_FRAME_RATE_CAP_FPS: f32 = 1.0;
pub(crate) const MAX_FRAME_RATE_CAP_FPS: f32 = 1000.0;
// OS timer wake-ups overshoot; wake this early, then poll to the deadline.
// Assumes 1ms system timer resolution (see timer_resolution.rs, active on
// Windows for the app lifetime); 2ms headroom is generous at that resolution.
pub(crate) const FRAME_WAKE_HEADROOM: Duration = Duration::from_millis(2);
// Used when the monitor refresh rate cannot be queried.
pub(crate) const FALLBACK_REFRESH_HZ: f32 = 60.0;

#[inline]
pub(crate) fn normalize_frame_rate_cap(cap: RuntimeFrameRateCap) -> RuntimeFrameRateCap {
    match cap {
        RuntimeFrameRateCap::Fps(fps) if fps.is_finite() && fps > 0.0 => {
            RuntimeFrameRateCap::Fps(fps.clamp(MIN_FRAME_RATE_CAP_FPS, MAX_FRAME_RATE_CAP_FPS))
        }
        RuntimeFrameRateCap::Fps(_) => RuntimeFrameRateCap::Unlimited,
        other => other,
    }
}

#[inline]
pub(crate) fn project_frame_rate_cap(cap: perro_runtime::FrameRateCap) -> RuntimeFrameRateCap {
    match cap {
        perro_runtime::FrameRateCap::Unlimited => RuntimeFrameRateCap::Unlimited,
        perro_runtime::FrameRateCap::Fps(fps) => RuntimeFrameRateCap::Fps(fps),
        perro_runtime::FrameRateCap::RefreshRate => RuntimeFrameRateCap::RefreshRate,
    }
}

#[inline]
pub(crate) fn frame_interval_from_fps(fps: f32) -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(fps))
}

/// Queries the OS for the active monitor refresh rate. Hits Win32/display
/// syscalls; call on window/monitor changes only, never per frame - the
/// pacer caches the result.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn query_refresh_rate_hz(window: Option<&Window>) -> Option<f32> {
    let window = window?;
    let monitor = window
        .current_monitor()
        .or_else(|| window.primary_monitor())?;
    let refresh_millihertz = monitor.refresh_rate_millihertz().or_else(|| {
        monitor
            .video_modes()
            .map(|mode| mode.refresh_rate_millihertz())
            .max()
    })?;
    if refresh_millihertz == 0 {
        return None;
    }
    Some(refresh_millihertz as f32 / 1000.0)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn query_refresh_rate_hz(_window: Option<&Window>) -> Option<f32> {
    Some(FALLBACK_REFRESH_HZ)
}

/// CPU-side frame pacer: owns the cap, the next frame deadline, and the
/// cached refresh rate. Knows about vsync so the CPU deadline and the
/// present block never fight over pacing.
pub(crate) struct FramePacer {
    cap: RuntimeFrameRateCap,
    vsync: bool,
    refresh_hz: Option<f32>,
    next_deadline: Option<Instant>,
}

impl FramePacer {
    pub(crate) fn new(cap: RuntimeFrameRateCap, vsync: bool) -> Self {
        Self {
            cap: normalize_frame_rate_cap(cap),
            vsync,
            refresh_hz: None,
            next_deadline: None,
        }
    }

    #[inline]
    pub(crate) fn cap(&self) -> RuntimeFrameRateCap {
        self.cap
    }

    /// Returns true when the cap actually changed; unchanged requests are a
    /// no-op so per-frame script calls neither spam logs nor re-anchor the
    /// deadline.
    pub(crate) fn set_cap(&mut self, cap: RuntimeFrameRateCap) -> bool {
        let cap = normalize_frame_rate_cap(cap);
        if cap == self.cap {
            return false;
        }
        self.cap = cap;
        self.next_deadline = None;
        true
    }

    /// Refresh the cached monitor rate. Call on resume, window move, and
    /// scale-factor change - the only times the active monitor can change.
    pub(crate) fn update_refresh_rate(&mut self, window: Option<&Window>) -> Option<f32> {
        self.refresh_hz = query_refresh_rate_hz(window);
        self.refresh_hz
    }

    #[inline]
    pub(crate) fn refresh_hz(&self) -> Option<f32> {
        self.refresh_hz
    }

    #[inline]
    fn refresh_interval(&self) -> Duration {
        frame_interval_from_fps(self.refresh_hz.unwrap_or(FALLBACK_REFRESH_HZ))
    }

    /// Raw cap interval, ignoring vsync.
    fn cap_interval(&self) -> Option<Duration> {
        match self.cap {
            RuntimeFrameRateCap::Unlimited => None,
            RuntimeFrameRateCap::Fps(fps) => Some(frame_interval_from_fps(fps)),
            RuntimeFrameRateCap::RefreshRate => Some(self.refresh_interval()),
        }
    }

    /// Interval the CPU deadline should enforce, or None when no CPU pacing
    /// is needed. With vsync on, the present block already paces at refresh,
    /// so a cap at or above refresh would only beat against it - skip it.
    fn pace_interval(&self, splash: bool) -> Option<Duration> {
        let refresh = self.refresh_interval();
        let interval = if splash {
            // Splash never needs more than refresh rate; honor a slower cap.
            Some(self.cap_interval().map_or(refresh, |cap| cap.max(refresh)))
        } else {
            self.cap_interval()
        }?;
        if self.vsync && interval <= refresh {
            return None;
        }
        Some(interval)
    }

    #[inline]
    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.next_deadline
    }

    #[inline]
    pub(crate) fn reset_deadline(&mut self) {
        self.next_deadline = None;
    }

    #[inline]
    pub(crate) fn blocks_frame(&self, now: Instant) -> bool {
        self.next_deadline.is_some_and(|deadline| deadline > now)
    }

    /// Advance the deadline after a frame. Deadlines chain from the previous
    /// deadline for drift-free pacing, re-anchoring from frame_start when the
    /// frame overran.
    pub(crate) fn update_deadline(
        &mut self,
        frame_start: Instant,
        frame_end: Instant,
        splash: bool,
    ) {
        let Some(interval) = self.pace_interval(splash) else {
            self.next_deadline = None;
            return;
        };
        let next = self
            .next_deadline
            .and_then(|deadline| deadline.checked_add(interval))
            .filter(|deadline| *deadline > frame_end)
            .unwrap_or_else(|| frame_start.checked_add(interval).unwrap_or(frame_end));
        self.next_deadline = Some(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_clamps_and_rejects_bad_fps() {
        assert_eq!(
            normalize_frame_rate_cap(RuntimeFrameRateCap::Fps(0.5)),
            RuntimeFrameRateCap::Fps(MIN_FRAME_RATE_CAP_FPS)
        );
        assert_eq!(
            normalize_frame_rate_cap(RuntimeFrameRateCap::Fps(9000.0)),
            RuntimeFrameRateCap::Fps(MAX_FRAME_RATE_CAP_FPS)
        );
        assert_eq!(
            normalize_frame_rate_cap(RuntimeFrameRateCap::Fps(f32::NAN)),
            RuntimeFrameRateCap::Unlimited
        );
    }

    #[test]
    fn set_cap_noop_when_unchanged() {
        let mut pacer = FramePacer::new(RuntimeFrameRateCap::Fps(120.0), false);
        assert!(!pacer.set_cap(RuntimeFrameRateCap::Fps(120.0)));
        assert!(pacer.set_cap(RuntimeFrameRateCap::Fps(60.0)));
    }

    #[test]
    fn vsync_skips_cpu_pacing_at_or_above_refresh() {
        let mut pacer = FramePacer::new(RuntimeFrameRateCap::RefreshRate, true);
        pacer.refresh_hz = Some(60.0);
        assert!(pacer.pace_interval(false).is_none());
        // Cap below refresh still needs the CPU deadline.
        assert!(pacer.set_cap(RuntimeFrameRateCap::Fps(30.0)));
        assert!(pacer.pace_interval(false).is_some());
        // Cap above refresh: present block is the slower pacer; skip.
        assert!(pacer.set_cap(RuntimeFrameRateCap::Fps(240.0)));
        assert!(pacer.pace_interval(false).is_none());
    }

    #[test]
    fn splash_paces_at_refresh_even_when_unlimited() {
        let mut pacer = FramePacer::new(RuntimeFrameRateCap::Unlimited, false);
        pacer.refresh_hz = Some(120.0);
        assert_eq!(
            pacer.pace_interval(true),
            Some(frame_interval_from_fps(120.0))
        );
        // Slower explicit cap wins over refresh during splash.
        pacer.set_cap(RuntimeFrameRateCap::Fps(30.0));
        assert_eq!(
            pacer.pace_interval(true),
            Some(frame_interval_from_fps(30.0))
        );
    }

    #[test]
    fn deadline_chains_and_reanchors() {
        let mut pacer = FramePacer::new(RuntimeFrameRateCap::Fps(100.0), false);
        let start = Instant::now();
        let end = start + Duration::from_millis(2);
        pacer.update_deadline(start, end, false);
        let first = pacer.deadline().unwrap();
        assert_eq!(first, start + Duration::from_millis(10));
        // Fast frame: next deadline chains from the previous one.
        let start2 = first;
        let end2 = first + Duration::from_millis(1);
        pacer.update_deadline(start2, end2, false);
        assert_eq!(pacer.deadline().unwrap(), first + Duration::from_millis(10));
        // Overrun past the chained deadline: re-anchor from frame start.
        let start3 = pacer.deadline().unwrap() + Duration::from_millis(50);
        let end3 = start3 + Duration::from_millis(1);
        pacer.update_deadline(start3, end3, false);
        assert_eq!(
            pacer.deadline().unwrap(),
            start3 + Duration::from_millis(10)
        );
    }
}
