//! Pipeline creation funnel.
//!
//! Every `create_render_pipeline` / `create_compute_pipeline` in the crate goes
//! through here so the descriptor's `cache` field has ONE owner. It is `None`
//! today: a persisted `wgpu::PipelineCache` was built and measured, and on a
//! Vulkan driver that keeps its own on-disk shader cache it bought nothing
//! while costing ~45ms of startup to open and seed the blob.
//!
//! Measured on Demo3D, `load_ready` (the pipeline-warm phase after first
//! present):
//!
//! ```text
//! first-ever launch (driver cache cold too)  +298ms
//! cold perro blob, warm driver cache          +62ms
//! warm perro blob                             +47ms / +54ms
//! cache path fully disabled                   +59ms / +58ms
//! ```
//!
//! Disabled is indistinguishable from enabled: the 298ms was the driver
//! compiling for the first time, which a per-app blob cannot reclaim on later
//! runs. The funnel stays so a cache can be reinstated at ONE site -- a driver
//! without its own shader cache would show a real difference -- instead of
//! re-touching ~60 pipeline builds across 20 files.

/// Cache handed to every pipeline build. See the module docs for why `None`.
#[inline]
fn cache() -> Option<&'static wgpu::PipelineCache> {
    None
}

pub(crate) fn create_render_pipeline(
    device: &wgpu::Device,
    descriptor: wgpu::RenderPipelineDescriptor<'_>,
) -> wgpu::RenderPipeline {
    let descriptor = wgpu::RenderPipelineDescriptor {
        cache: cache(),
        ..descriptor
    };
    device.create_render_pipeline(&descriptor)
}

pub(crate) fn create_compute_pipeline(
    device: &wgpu::Device,
    descriptor: wgpu::ComputePipelineDescriptor<'_>,
) -> wgpu::ComputePipeline {
    let descriptor = wgpu::ComputePipelineDescriptor {
        cache: cache(),
        ..descriptor
    };
    device.create_compute_pipeline(&descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reinstating a cache must come with the measurement that justifies it; a
    /// `Some` here silently adds the startup cost back.
    #[test]
    fn no_cache_is_bound() {
        assert!(cache().is_none());
    }
}
