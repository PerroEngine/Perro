//! Per-frame event counters for `PERRO_SPIKE_LOG` hitch diagnosis.
//!
//! Relaxed atomic adds at rare events only (stream create/resume/alloc,
//! mesh/texture upload). The runner drains them once per frame with `take()`.
//! Pipeline builds are counted at the engine's creation funnel. Do not call
//! `Device::get_internal_counters` here: on some Vulkan drivers that probe can
//! stall for tens of milliseconds and turn the hitch logger into the hitch.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

macro_rules! counters {
    ($($name:ident),* $(,)?) => {
        $(static $name: AtomicU64 = AtomicU64::new(0);)*
    };
}

counters!(
    STREAM_UPSERTS,
    STREAM_CHANGED,
    STREAM_NEW,
    STREAM_REMOVES,
    STREAM_SUSPENDS,
    STREAM_RESUMES,
    STREAM_TEX_RESIZES,
    STREAM_RT_ALLOCS,
    STREAM_RT_BYTES,
    TEX_UPLOADS,
    TEX_UPLOAD_BYTES,
    MESH_UPLOADS,
    MESH_UPLOAD_BYTES,
    RENDER_PIPELINE_BUILDS,
    COMPUTE_PIPELINE_BUILDS,
);

/// Event counts since the previous `take()`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GfxSpikeCounters {
    pub stream_upserts: u64,
    pub stream_changed: u64,
    pub stream_new: u64,
    pub stream_removes: u64,
    pub stream_suspends: u64,
    pub stream_resumes: u64,
    pub stream_tex_resizes: u64,
    pub stream_rt_allocs: u64,
    pub stream_rt_bytes: u64,
    pub tex_uploads: u64,
    pub tex_upload_bytes: u64,
    pub mesh_uploads: u64,
    pub mesh_upload_bytes: u64,
    pub render_pipeline_builds: u64,
    pub compute_pipeline_builds: u64,
}

/// `PERRO_SPIKE_LOG` set to a positive ms threshold; read once.
pub fn spike_log_threshold_ms() -> Option<f64> {
    static THRESHOLD: OnceLock<Option<f64>> = OnceLock::new();
    *THRESHOLD.get_or_init(|| {
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::env::var("PERRO_SPIKE_LOG")
                .ok()
                .and_then(|raw| raw.trim().parse::<f64>().ok())
                .filter(|ms| ms.is_finite() && *ms > 0.0)
        }
    })
}

#[inline]
pub fn spike_log_enabled() -> bool {
    spike_log_threshold_ms().is_some()
}

#[inline]
fn bump(counter: &AtomicU64, by: u64) {
    if spike_log_enabled() {
        counter.fetch_add(by, Ordering::Relaxed);
    }
}

#[inline]
pub(crate) fn render_pipeline_build() {
    bump(&RENDER_PIPELINE_BUILDS, 1);
}

#[inline]
pub(crate) fn compute_pipeline_build() {
    bump(&COMPUTE_PIPELINE_BUILDS, 1);
}

#[inline]
pub(crate) fn stream_upsert(changed: bool) {
    bump(&STREAM_UPSERTS, 1);
    if changed {
        bump(&STREAM_CHANGED, 1);
    }
}
#[inline]
pub(crate) fn stream_new() {
    bump(&STREAM_NEW, 1);
}
#[inline]
pub(crate) fn stream_remove() {
    bump(&STREAM_REMOVES, 1);
}
#[inline]
pub(crate) fn stream_suspend() {
    bump(&STREAM_SUSPENDS, 1);
}
#[inline]
pub(crate) fn stream_resume() {
    bump(&STREAM_RESUMES, 1);
}
#[inline]
pub(crate) fn stream_tex_resize() {
    bump(&STREAM_TEX_RESIZES, 1);
}
#[inline]
pub(crate) fn stream_rt_alloc(bytes: u64) {
    bump(&STREAM_RT_ALLOCS, 1);
    bump(&STREAM_RT_BYTES, bytes);
}
#[inline]
pub(crate) fn tex_upload(new_texture: bool, bytes: u64) {
    if new_texture {
        bump(&TEX_UPLOADS, 1);
    }
    bump(&TEX_UPLOAD_BYTES, bytes);
}
#[inline]
pub(crate) fn mesh_upload(bytes: u64) {
    bump(&MESH_UPLOADS, 1);
    bump(&MESH_UPLOAD_BYTES, bytes);
}

/// Drain the event counters (reset to zero).
pub fn take() -> GfxSpikeCounters {
    let t = |c: &AtomicU64| c.swap(0, Ordering::Relaxed);
    GfxSpikeCounters {
        stream_upserts: t(&STREAM_UPSERTS),
        stream_changed: t(&STREAM_CHANGED),
        stream_new: t(&STREAM_NEW),
        stream_removes: t(&STREAM_REMOVES),
        stream_suspends: t(&STREAM_SUSPENDS),
        stream_resumes: t(&STREAM_RESUMES),
        stream_tex_resizes: t(&STREAM_TEX_RESIZES),
        stream_rt_allocs: t(&STREAM_RT_ALLOCS),
        stream_rt_bytes: t(&STREAM_RT_BYTES),
        tex_uploads: t(&TEX_UPLOADS),
        tex_upload_bytes: t(&TEX_UPLOAD_BYTES),
        mesh_uploads: t(&MESH_UPLOADS),
        mesh_upload_bytes: t(&MESH_UPLOAD_BYTES),
        render_pipeline_builds: t(&RENDER_PIPELINE_BUILDS),
        compute_pipeline_builds: t(&COMPUTE_PIPELINE_BUILDS),
    }
}
