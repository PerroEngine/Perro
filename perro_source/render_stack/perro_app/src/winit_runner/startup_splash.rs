#[cfg(not(target_arch = "wasm32"))]
use super::image_helpers::load_image_sizes;
use perro_ids::{NodeID, TextureID, string_to_u64};
use perro_render_bridge::RenderRequestID;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

pub(super) const STARTUP_SPLASH_FADE_DURATION: Duration = Duration::from_millis(320);
pub(super) const STARTUP_SPLASH_HOLD_DURATION: Duration = Duration::from_millis(2000);
pub(super) const STARTUP_SPLASH_HARD_TIMEOUT: Duration = Duration::from_millis(8000);
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

pub(super) struct StartupSplashState {
    pub(super) active: bool,
    pub(super) source: Option<String>,
    pub(super) source_hash: Option<u64>,
    pub(super) image_size: Option<(u32, u32)>,
    pub(super) texture_size: Option<(u32, u32)>,
    pub(super) texture_requested: bool,
    pub(super) texture_id: Option<TextureID>,
    pub(super) ready_streak: u32,
    pub(super) shown_at: Instant,
    pub(super) fade_started_at: Option<Instant>,
    pub(super) first_frame_inflight: Vec<RenderRequestID>,
    pub(super) first_frame_captured: bool,
}

impl StartupSplashState {
    pub(super) fn from_project(
        project: Option<&perro_runtime::RuntimeProject>,
        now: Instant,
    ) -> Self {
        #[cfg(target_arch = "wasm32")]
        let _ = project;
        #[cfg(target_arch = "wasm32")]
        let splash = None::<(String, Option<u64>, Option<(u32, u32)>, Option<(u32, u32)>)>;
        #[cfg(not(target_arch = "wasm32"))]
        let splash = {
            let mut source = None::<String>;
            let mut source_hash = None::<u64>;
            if let Some(p) = project {
                let splash = p.config.startup_splash.trim();
                if !splash.is_empty() {
                    source = Some(splash.to_string());
                    source_hash = p.config.startup_splash_hash;
                } else {
                    let icon = p.config.icon.trim();
                    if !icon.is_empty() {
                        source = Some(icon.to_string());
                        source_hash = p.config.icon_hash;
                    }
                }
            }
            let image_sizes = project.and_then(|p| {
                source
                    .as_deref()
                    .and_then(|s| load_image_sizes(p, s, source_hash))
            });
            source.map(|source| {
                (
                    source,
                    source_hash,
                    image_sizes.map(|sizes| sizes.display),
                    image_sizes.map(|sizes| sizes.texture),
                )
            })
        };
        let (
            active,
            source,
            source_hash,
            image_size,
            texture_size,
            fade_started_at,
            first_frame_captured,
        ) = if let Some((source, source_hash, image_size, texture_size)) = splash {
            (
                true,
                Some(source),
                source_hash,
                image_size,
                texture_size,
                None,
                false,
            )
        } else {
            (false, None, None, None, None, Some(now), true)
        };
        Self {
            active,
            source,
            source_hash,
            image_size,
            texture_size,
            texture_requested: false,
            texture_id: None,
            ready_streak: 0,
            shown_at: now,
            fade_started_at,
            first_frame_inflight: Vec::new(),
            first_frame_captured,
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
