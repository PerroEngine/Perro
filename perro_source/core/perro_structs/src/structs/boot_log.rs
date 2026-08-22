//! `PERRO_BOOT_LOG=1` startup phase trace.
//!
//! Startup is the one stretch the per-frame timing CSV cannot explain: it only
//! starts sampling once frames are being presented, so everything before the
//! first present (project parse, script registry, wgpu device, window, initial
//! asset upload) is a single opaque gap. This marks the milestones with a
//! monotonic clock started at process entry.
//!
//! Off unless the env var is set: one relaxed atomic load per mark.

use std::sync::OnceLock;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

/// Emit one boot line. wasm32-unknown-unknown has no stderr, so the browser
/// console is the only channel that reaches a developer there.
macro_rules! boot_line {
    ($($arg:tt)*) => {{
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!($($arg)*)));
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!($($arg)*);
    }};
}

static ENABLED: OnceLock<bool> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();
static LAST: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);
static LAST_STALL: std::sync::Mutex<Option<(String, u64)>> = std::sync::Mutex::new(None);

fn enabled() -> bool {
    // No env vars in a browser, so the page URL carries the opt-in:
    // append `?perro_boot_log` (or `&perro_boot_log`) to the demo URL.
    #[cfg(target_arch = "wasm32")]
    {
        *ENABLED.get_or_init(|| {
            web_sys::window()
                .and_then(|window| window.location().search().ok())
                .is_some_and(|search| search.contains("perro_boot_log"))
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        *ENABLED.get_or_init(|| std::env::var_os("PERRO_BOOT_LOG").is_some())
    }
}

/// Start the boot clock. Call as early in `main` as the entry point allows;
/// later calls are ignored, so the earliest caller wins.
pub fn start() {
    if !enabled() {
        return;
    }
    let now = Instant::now();
    let _ = START.set(now);
    if let Ok(mut last) = LAST.lock() {
        last.get_or_insert(now);
    }
}

/// Mark a startup milestone: total elapsed since [`start`] plus the delta from
/// the previous mark, which is the phase that just finished.
pub fn mark(phase: &str) {
    if !enabled() {
        return;
    }
    let now = Instant::now();
    let start = *START.get_or_init(|| now);
    let previous = LAST
        .lock()
        .ok()
        .and_then(|mut last| last.replace(now))
        .unwrap_or(start);
    boot_line!(
        "[perro][boot] {phase} +{:.0}ms (total {:.0}ms)",
        now.duration_since(previous).as_secs_f64() * 1000.0,
        now.duration_since(start).as_secs_f64() * 1000.0,
    );
}

/// Report a phase that is taking too long, at most once per whole second.
///
/// For gates that can stall silently: nothing logs while a boot gate sits
/// false, so a hang is indistinguishable from slow work. `elapsed` is how long
/// the phase has run; `state` is only formatted when a line is actually
/// emitted, so the caller can build a rich string without paying for it.
pub fn stall(label: &str, elapsed: std::time::Duration, state: impl FnOnce() -> String) {
    if !enabled() || elapsed < std::time::Duration::from_secs(1) {
        return;
    }
    let secs = elapsed.as_secs();
    let Ok(mut last) = LAST_STALL.lock() else {
        return;
    };
    if last
        .as_ref()
        .is_some_and(|(seen, at)| seen == label && *at == secs)
    {
        return;
    }
    *last = Some((label.to_string(), secs));
    drop(last);
    boot_line!("[perro][boot] {label} still waiting @{secs}s: {}", state());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Marking without an explicit `start` must not panic or divide by a clock
    /// that was never set; the first mark becomes the origin.
    #[test]
    fn mark_without_start_is_safe() {
        mark("test_phase");
    }

    /// Disabled is the default, so a normal run pays one atomic load per mark.
    #[test]
    fn disabled_unless_env_set() {
        if std::env::var_os("PERRO_BOOT_LOG").is_none() {
            assert!(!enabled());
        }
    }
}
