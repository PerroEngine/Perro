//! THE supersample factor. One number, one owner, every render target.
//!
//! # Single-supersample invariant
//!
//! For any render target `T`:
//!
//! ```text
//! effective_supersample(T) == own_factor(T)
//! ```
//!
//! `own_factor(T)` is the factor `T` derives from ITS OWN on-screen size (or
//! the resolution its author pinned). It never depends on nesting depth, and
//! never on any ancestor's factor. Two targets in a parent/child chain each
//! supersample once; the chain does NOT multiply.
//!
//! Two independent sites used to pick "2" by hand and their equality is what
//! makes a depth-0 sub-view land 1:1 inside the UI raster target:
//!
//! - `perro_graphics` rasterizes UI at `scale * viewport` and minifies it back
//!   in the composite pass.
//! - `perro_runtime` sizes an auto-resolution sub-view target at
//!   `scale * ui_rect`.
//!
//! Equal factors => the sub-view's footprint inside the UI raster target equals
//! its own target => ONE resolve, not two stacked. Measured at 1920x1080 with a
//! 829x467 sub-view rect: footprint 1658x934 vs target 1664x937 (the 6px is
//! long-axis bucket rounding), ratio 1.004. Total 2.007x linear / 4.03x pixels
//! = exactly one 2x supersample.
//!
//! Both sites now read [`supersample_scale`]. Do not reintroduce a local
//! constant at either end.
//!
//! # Why the factor is an INTEGER
//!
//! UI layout runs at scale factor 1.0 and `pixel_snapping` snaps edges in that
//! layout space. A snapped edge at `N` rasterizes at `N * scale` and resolves
//! back to exactly `N` only while `scale` is a whole number. A fractional
//! factor would put snapped edges back on half-pixels, which is the artifact
//! snapping exists to remove.
//!
//! # Why `Nearest` drops to 1
//!
//! The UI composite samples the raster target through the project's
//! `TextureFilterMode` sampler. Under `Nearest` it point-samples the 2x target
//! down: 4x the raster cost, zero AA benefit, plus the aliasing that
//! point-sampled minification produces. A pixel-art project wants crisp pixels,
//! not AA, so the supersample is dropped instead of the composite being forced
//! to linear behind the author's back. Both factors become 1 together, so the
//! 1:1 landing above still holds.

use super::TextureFilterMode;

/// Linear supersample factor for filter modes whose composite sampler
/// interpolates. INTEGER by contract (see module docs).
pub const SUPERSAMPLE_SCALE: u32 = 2;

/// The one supersample factor for a project, keyed on its texture filter mode.
///
/// Every render target that derives a size from an on-screen size must scale by
/// exactly this, exactly once. Callers that need a float multiplier use
/// [`supersample_scale_f32`].
#[inline]
pub const fn supersample_scale(filter: TextureFilterMode) -> u32 {
    match filter {
        // Point sampling gains nothing from a supersampled source and costs
        // `SUPERSAMPLE_SCALE^2` pixels to produce it.
        TextureFilterMode::Nearest => 1,
        TextureFilterMode::Linear
        | TextureFilterMode::LinearMipmap
        | TextureFilterMode::Anisotropic => SUPERSAMPLE_SCALE,
    }
}

/// [`supersample_scale`] as the float multiplier target-sizing math wants.
/// Exact: every value the `u32` form yields is representable in `f32`.
#[inline]
pub const fn supersample_scale_f32(filter: TextureFilterMode) -> f32 {
    supersample_scale(filter) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pixel-snapping invariant depends on a whole-number factor: a snapped
    /// layout edge at N must land back on exactly N after raster + resolve.
    #[test]
    fn every_mode_yields_a_whole_number_factor_of_at_least_one() {
        for mode in [
            TextureFilterMode::Nearest,
            TextureFilterMode::Linear,
            TextureFilterMode::LinearMipmap,
            TextureFilterMode::Anisotropic,
        ] {
            let scale = supersample_scale(mode);
            assert!(scale >= 1, "{mode:?} must never shrink a target");
            assert_eq!(supersample_scale_f32(mode), scale as f32);
            assert_eq!(supersample_scale_f32(mode).fract(), 0.0);
        }
    }

    /// Point-sampled projects pay 4x raster for zero AA, so they opt out.
    #[test]
    fn nearest_opts_out_and_filtering_modes_opt_in() {
        assert_eq!(supersample_scale(TextureFilterMode::Nearest), 1);
        for mode in [
            TextureFilterMode::Linear,
            TextureFilterMode::LinearMipmap,
            TextureFilterMode::Anisotropic,
        ] {
            assert_eq!(supersample_scale(mode), SUPERSAMPLE_SCALE);
        }
    }
}
