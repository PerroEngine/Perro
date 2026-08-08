//! Remembers which graphics backend the full adapter enumeration picked.
//!
//! `Instance::new` with every compiled backend enabled makes `request_adapter`
//! initialize each one before choosing. On a Windows box with both DX12 and
//! Vulkan drivers that costs ~450ms of boot to enumerate DX12 adapters that are
//! then discarded, because the enumeration picks Vulkan anyway.
//!
//! So: record the backend the full enumeration chose, and on the next run build
//! the instance with only that backend. This is a replay, not a preference --
//! the selection is whatever wgpu itself decided on this machine, so behaviour
//! is unchanged and only the discarded work disappears. A hardcoded per-OS
//! order would instead change which GPU some machines run on.
//!
//! Every failure path falls back to full enumeration and re-records:
//! unreadable/absent/garbage cache, a backend that no longer yields an adapter
//! (driver uninstalled, eGPU unplugged, new hardware). `WGPU_BACKEND` bypasses
//! the cache entirely so the env override keeps working.

use std::path::PathBuf;

const CACHE_FILE: &str = "gpu_backend";
const CACHE_VERSION: u32 = 1;

fn cache_path() -> Option<PathBuf> {
    Some(perro_io::dirs::cache_dir()?.join("perro").join(CACHE_FILE))
}

/// Serialized backend name. Only the variants wgpu can select natively.
fn backend_name(backend: wgpu::Backend) -> Option<&'static str> {
    match backend {
        wgpu::Backend::Vulkan => Some("vulkan"),
        wgpu::Backend::Dx12 => Some("dx12"),
        wgpu::Backend::Metal => Some("metal"),
        wgpu::Backend::Gl => Some("gl"),
        _ => None,
    }
}

fn backend_from_name(name: &str) -> Option<wgpu::Backends> {
    match name {
        "vulkan" => Some(wgpu::Backends::VULKAN),
        "dx12" => Some(wgpu::Backends::DX12),
        "metal" => Some(wgpu::Backends::METAL),
        "gl" => Some(wgpu::Backends::GL),
        _ => None,
    }
}

/// The backend a previous run selected, if it is still worth trying.
///
/// Returns `None` when `WGPU_BACKEND` is set: the user asked for a specific
/// backend and `with_env` already honours that.
pub(crate) fn cached_backends() -> Option<wgpu::Backends> {
    if std::env::var_os("WGPU_BACKEND").is_some() {
        return None;
    }
    let text = std::fs::read_to_string(cache_path()?).ok()?;
    let mut parts = text.split_whitespace();
    if parts.next()? != format!("v{CACHE_VERSION}") {
        return None;
    }
    backend_from_name(parts.next()?)
}

/// Records the backend the full enumeration chose. Best-effort: a write failure
/// only costs the next run its enumeration.
pub(crate) fn store_backend(backend: wgpu::Backend) {
    if std::env::var_os("WGPU_BACKEND").is_some() {
        return;
    }
    let Some(name) = backend_name(backend) else {
        return;
    };
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, format!("v{CACHE_VERSION} {name}\n"));
}

/// Drops a cache entry that no longer resolves to an adapter.
pub(crate) fn clear() {
    if let Some(path) = cache_path() {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_supported_backend() {
        for (backend, name) in [
            (wgpu::Backend::Vulkan, "vulkan"),
            (wgpu::Backend::Dx12, "dx12"),
            (wgpu::Backend::Metal, "metal"),
            (wgpu::Backend::Gl, "gl"),
        ] {
            assert_eq!(backend_name(backend), Some(name));
            assert!(backend_from_name(name).is_some());
        }
    }

    #[test]
    fn rejects_unknown_and_noop_backends() {
        assert_eq!(backend_name(wgpu::Backend::Noop), None);
        assert_eq!(backend_from_name("wgpu-of-the-future"), None);
        assert_eq!(backend_from_name(""), None);
    }
}
