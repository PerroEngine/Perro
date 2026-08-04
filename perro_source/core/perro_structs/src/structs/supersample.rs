//! Shared UI + auto-`UiSubView` raster scale.
//!
//! # Single-scale invariant
//!
//! Main UI and auto-resolution `UiSubView` targets both derive from this one
//! value. It is fixed at 1x for every texture filter mode: UI rasterizes at
//! output resolution and an auto sub-view starts from its own logical rect.
//! Nesting must not multiply the factor or inherit an ancestor factor.
//!
//! Auto sub-view bucketing may round a target up to the next 64px long-axis
//! bucket. That allocation slack is not an extra raster scale policy.
//!
//! Keep both call sites on [`supersample_scale`]. Do not add a local UI or
//! runtime multiplier.

use super::TextureFilterMode;

/// Shared raster scale. Fixed at 1x: no filter mode enables supersampling.
pub const SUPERSAMPLE_SCALE: u32 = 1;

/// Shared UI + auto-sub-view raster scale.
///
/// Keep the filter parameter for API stability and to keep both consumers on
/// one policy function. Texture filtering changes sampling, not target size.
#[inline]
pub const fn supersample_scale(_filter: TextureFilterMode) -> u32 {
    SUPERSAMPLE_SCALE
}

/// [`supersample_scale`] as the float multiplier target-sizing math wants.
#[inline]
pub const fn supersample_scale_f32(filter: TextureFilterMode) -> f32 {
    supersample_scale(filter) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_filter_mode_uses_one_x() {
        for mode in [
            TextureFilterMode::Nearest,
            TextureFilterMode::Linear,
            TextureFilterMode::LinearMipmap,
            TextureFilterMode::Anisotropic,
        ] {
            assert_eq!(supersample_scale(mode), 1);
            assert_eq!(supersample_scale_f32(mode), 1.0);
        }
    }
}
