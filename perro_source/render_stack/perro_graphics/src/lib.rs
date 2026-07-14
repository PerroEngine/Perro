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
    DrawFrameTiming, GraphicsBackend, OcclusionCullingMode, PerroGraphics, SsaoQuality,
    StaticMeshLookup, StaticShaderLookup, StaticTextureLookup,
};
pub use resources::{ResourceGcDrops, ResourceStore};

/// Emissive packs normalized rgb + max-component/EMISSIVE_PACK_MAX in unorm8
/// lanes; shaders decode `rgb * w * EMISSIVE_PACK_MAX`.
pub(crate) const EMISSIVE_PACK_MAX: f32 = 16.0;

/// Scene depth format shared by every pipeline that attaches the 3D scene
/// depth target (meshes, multimesh, water, 3D particles). Depth32Float at
/// sample_count == 1 so the depth prepass result can be copied into the main
/// depth target and reused by the opaque pass; Depth24Plus under MSAA where
/// the 1-sample prepass cannot be shared and depth copies are not allowed.
pub(crate) const fn scene_depth_format(sample_count: u32) -> wgpu::TextureFormat {
    if sample_count <= 1 {
        wgpu::TextureFormat::Depth32Float
    } else {
        wgpu::TextureFormat::Depth24Plus
    }
}

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
