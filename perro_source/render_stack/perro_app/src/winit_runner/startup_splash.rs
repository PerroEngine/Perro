#[cfg(not(target_arch = "wasm32"))]
use super::image_helpers::PreloadedStartupSplash;
use perro_ids::{NodeID, TextureID, string_to_u64};
use perro_render_bridge::RenderRequestID;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

pub(super) const STARTUP_SPLASH_FADE_DURATION: Duration = Duration::from_millis(320);
pub(super) const STARTUP_SPLASH_HOLD_DURATION: Duration = Duration::from_millis(2000);
// Terminal bound on the whole splash. Past this the fade starts regardless of
// every other gate: a pathological compile, a boot scene that never reports
// loaded, or a render request that never completes must not pin the splash
// forever. A partly-warm first frame beats a window that never opens.
pub(super) const STARTUP_SPLASH_HARD_TIMEOUT: Duration = Duration::from_millis(10_000);
pub(super) const STARTUP_SPLASH_BG_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
pub(super) const STARTUP_SPLASH_MAX_WIDTH_FRAC: f32 = 0.44;
pub(super) const STARTUP_SPLASH_MAX_HEIGHT_FRAC: f32 = 0.34;
pub(super) const STARTUP_SPLASH_TEXTURE_REQUEST: RenderRequestID =
    RenderRequestID::new(0x5350_4C41_5348_5F54);
pub(super) const STARTUP_SPLASH_BG_NODE: NodeID =
    NodeID::from_u64(string_to_u64("__startup_splash_bg__"));
pub(super) const STARTUP_SPLASH_IMAGE_NODE: NodeID =
    NodeID::from_u64(string_to_u64("__startup_splash_image__"));
pub(super) const STARTUP_SPLASH_BG_Z: i32 = 950;
pub(super) const STARTUP_SPLASH_IMAGE_Z: i32 = 951;

#[inline]
pub(super) fn next_ready_streak(current: u32, presented: bool, assets_ready: bool) -> u32 {
    if presented && assets_ready {
        current.saturating_add(1)
    } else {
        0
    }
}

#[inline]
pub(super) fn boot_load_may_start(splash_active: bool, window_visible: bool) -> bool {
    !splash_active || window_visible
}

/// Should the splash start fading this frame?
///
/// Two independent ways out, and the second must not be an extra AND-term:
/// the normal path (branding hold done AND everything loaded), OR the hard
/// timeout on its own. Folding the timeout into `load_ready` only relaxed the
/// pipeline-warm term, so a boot scene that never reported loaded -- or a
/// `ready_streak` reset every frame by one never-completing render request --
/// pinned the splash with no way out.
#[inline]
pub(super) fn splash_fade_should_start(
    shown_for: Duration,
    load_ready: bool,
    hard_timeout_hit: bool,
) -> bool {
    (shown_for >= STARTUP_SPLASH_HOLD_DURATION && load_ready) || hard_timeout_hit
}

pub(super) struct StartupSplashState {
    pub(super) active: bool,
    pub(super) source: Option<String>,
    pub(super) source_hash: Option<u64>,
    pub(super) image_size: Option<(u32, u32)>,
    pub(super) texture_size: Option<(u32, u32)>,
    pub(super) rgba: Option<std::sync::Arc<[u8]>>,
    pub(super) texture_requested: bool,
    pub(super) texture_id: Option<TextureID>,
    pub(super) ready_streak: u32,
    pub(super) shown_at: Instant,
    pub(super) fade_started_at: Option<Instant>,
    pub(super) first_frame_inflight: Vec<RenderRequestID>,
    pub(super) first_frame_captured: bool,
}

impl StartupSplashState {
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn from_preloaded(preload: Option<PreloadedStartupSplash>, now: Instant) -> Self {
        let splash = preload.map(|splash| {
            (
                splash.source,
                splash.source_hash,
                splash.image_size,
                splash.texture_size,
                splash.rgba,
            )
        });
        let (
            active,
            source,
            source_hash,
            image_size,
            texture_size,
            rgba,
            fade_started_at,
            first_frame_captured,
        ) = if let Some((source, source_hash, image_size, texture_size, rgba)) = splash {
            (
                true,
                Some(source),
                source_hash,
                image_size,
                texture_size,
                rgba,
                None,
                false,
            )
        } else {
            (false, None, None, None, None, None, Some(now), true)
        };
        Self {
            active,
            source,
            source_hash,
            image_size,
            texture_size,
            rgba,
            texture_requested: false,
            texture_id: None,
            ready_streak: 0,
            shown_at: now,
            fade_started_at,
            first_frame_inflight: Vec::new(),
            first_frame_captured,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn from_preloaded(now: Instant) -> Self {
        Self {
            active: false,
            source: None,
            source_hash: None,
            image_size: None,
            texture_size: None,
            rgba: None,
            texture_requested: false,
            texture_id: None,
            ready_streak: 0,
            shown_at: now,
            fade_started_at: Some(now),
            first_frame_inflight: Vec::new(),
            first_frame_captured: true,
        }
    }

    #[inline]
    pub(super) fn blocks_input(&self) -> bool {
        self.active && !self.first_frame_captured
    }

    pub(super) fn alpha(&self, now: Instant) -> f32 {
        let Some(started) = self.fade_started_at else {
            return 1.0;
        };
        let elapsed = now.saturating_duration_since(started);
        if elapsed >= STARTUP_SPLASH_FADE_DURATION {
            0.0
        } else {
            1.0 - (elapsed.as_secs_f32() / STARTUP_SPLASH_FADE_DURATION.as_secs_f32())
        }
    }

    pub(super) fn should_finish(&self, now: Instant) -> bool {
        self.fade_started_at.is_some_and(|started| {
            now.saturating_duration_since(started) >= STARTUP_SPLASH_FADE_DURATION
        })
    }
}

#[cfg(test)]
mod tests {
    use super::next_ready_streak;

    #[test]
    fn ready_streak_needs_two_presented_asset_ready_frames() {
        let first = next_ready_streak(0, true, true);
        let second = next_ready_streak(first, true, true);
        assert_eq!(first, 1);
        assert_eq!(second, 2);
    }

    #[test]
    fn failed_present_resets_ready_streak() {
        assert_eq!(next_ready_streak(1, false, true), 0);
        assert_eq!(next_ready_streak(1, true, false), 0);
    }

    /// The regression this guards: the timeout was ANDed into `load_ready`
    /// instead of ORed with it, so a stuck gate hung the splash past 60s with
    /// no escape. Past the hard timeout the fade starts whatever else is false.
    #[test]
    fn hard_timeout_starts_the_fade_even_when_nothing_is_ready() {
        assert!(super::splash_fade_should_start(
            super::STARTUP_SPLASH_HARD_TIMEOUT,
            false,
            true
        ));
        assert!(super::splash_fade_should_start(
            super::STARTUP_SPLASH_HARD_TIMEOUT * 6,
            false,
            true
        ));
    }

    /// The normal path still needs BOTH the branding hold and a ready load, so
    /// a fast boot cannot cut the splash short.
    #[test]
    fn normal_exit_needs_hold_and_load_ready() {
        use std::time::Duration;
        assert!(!super::splash_fade_should_start(
            Duration::from_millis(100),
            true,
            false
        ));
        assert!(!super::splash_fade_should_start(
            super::STARTUP_SPLASH_HOLD_DURATION,
            false,
            false
        ));
        assert!(super::splash_fade_should_start(
            super::STARTUP_SPLASH_HOLD_DURATION,
            true,
            false
        ));
    }

    #[test]
    fn boot_load_waits_for_visible_splash() {
        assert!(!super::boot_load_may_start(true, false));
        assert!(super::boot_load_may_start(true, true));
        assert!(super::boot_load_may_start(false, false));
    }
}
