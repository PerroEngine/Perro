//! Per-frame runtime event counters for `PERRO_SPIKE_LOG` hitch diagnosis.
//! Relaxed atomic adds only; the app runner drains them with `take()`.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

static NODES_CREATED: AtomicU64 = AtomicU64::new(0);
static NODES_REMOVED: AtomicU64 = AtomicU64::new(0);
static SCENE_LOADS: AtomicU64 = AtomicU64::new(0);
static SCENE_LOAD_NS: AtomicU64 = AtomicU64::new(0);

#[inline]
fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("PERRO_SPIKE_LOG")
            .ok()
            .and_then(|raw| raw.trim().parse::<f64>().ok())
            .is_some_and(|ms| ms.is_finite() && ms > 0.0)
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeSpikeCounters {
    pub nodes_created: u64,
    pub nodes_removed: u64,
    pub scene_loads: u64,
    /// Wall time inside runtime scene loads (nested loads count twice).
    pub scene_load_ns: u64,
}

#[inline]
pub(crate) fn node_created() {
    if enabled() {
        NODES_CREATED.fetch_add(1, Ordering::Relaxed);
    }
}

#[inline]
pub(crate) fn node_removed() {
    if enabled() {
        NODES_REMOVED.fetch_add(1, Ordering::Relaxed);
    }
}

/// Times one scene load; records on drop (covers `?` early returns).
pub(crate) struct SceneLoadTimer(Option<Instant>);

impl SceneLoadTimer {
    #[inline]
    pub(crate) fn start() -> Self {
        Self(enabled().then(Instant::now))
    }
}

impl Drop for SceneLoadTimer {
    fn drop(&mut self) {
        let Some(start) = self.0 else {
            return;
        };
        SCENE_LOADS.fetch_add(1, Ordering::Relaxed);
        let ns = start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        SCENE_LOAD_NS.fetch_add(ns, Ordering::Relaxed);
    }
}

/// Drain counters since the previous call.
pub fn take() -> RuntimeSpikeCounters {
    RuntimeSpikeCounters {
        nodes_created: NODES_CREATED.swap(0, Ordering::Relaxed),
        nodes_removed: NODES_REMOVED.swap(0, Ordering::Relaxed),
        scene_loads: SCENE_LOADS.swap(0, Ordering::Relaxed),
        scene_load_ns: SCENE_LOAD_NS.swap(0, Ordering::Relaxed),
    }
}
