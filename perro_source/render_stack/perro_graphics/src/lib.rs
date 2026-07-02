mod backend;
mod gpu;
mod postprocess;
mod resources;
mod texture_mips;
pub mod three_d;
pub mod two_d;
pub mod ui;
mod visual_accessibility;

pub use backend::{
    DrawFrameTiming, GraphicsBackend, OcclusionCullingMode, PerroGraphics, StaticMeshLookup,
    StaticShaderLookup, StaticTextureLookup,
};
pub use resources::{ResourceGcDrops, ResourceStore};

/// Emissive packs normalized rgb + max-component/EMISSIVE_PACK_MAX in unorm8
/// lanes; shaders decode `rgb * w * EMISSIVE_PACK_MAX`.
pub(crate) const EMISSIVE_PACK_MAX: f32 = 16.0;

/// Decode an sRGB-encoded channel to linear. Authored colors (pickers, hex)
/// are sRGB; lighting runs in linear. Values above 1 pass through untouched.
pub(crate) fn srgb_channel_to_linear(c: f32) -> f32 {
    if !c.is_finite() || c <= 0.0 {
        0.0
    } else if c <= 0.04045 {
        c / 12.92
    } else if c <= 1.0 {
        ((c + 0.055) / 1.055).powf(2.4)
    } else {
        c
    }
}

pub(crate) fn srgb_to_linear_rgb(rgb: [f32; 3]) -> [f32; 3] {
    [
        srgb_channel_to_linear(rgb[0]),
        srgb_channel_to_linear(rgb[1]),
        srgb_channel_to_linear(rgb[2]),
    ]
}
