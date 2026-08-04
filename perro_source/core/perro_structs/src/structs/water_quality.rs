//! Water fidelity tier: 1 knob per water body.
//!
//! # Why 1 tier
//!
//! Old water carried 6 absolute knobs (`resolution`, `render_resolution`,
//! `lod_min_resolution`, `lod_near/mid/far_distance`). Absolute world-grid
//! sizes get 4 things wrong at once: body size, camera distance, window size,
//! `render_scale`. A 256x256 grid on a 100m plane tessellates the far half as
//! finely as the near half.
//!
//! # Screen-space rule
//!
//! Tier maps to a target triangle edge length in PIXELS of the render target.
//! Per chunk, per frame, tessellation derives from chunk distance + projection
//! scale + target height. Same tier => same on-screen triangle density on every
//! body, every camera, every window, every `render_scale`.
//!
//! Tier also drives the sim grid + physics sample readback rate, so 1 field
//! moves every fidelity axis together.
//!
//! Default = [`WaterQuality::Low`]. Authors opt IN to more.

use core::fmt;

/// Per-body water fidelity tier. Default = [`Self::Low`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum WaterQuality {
    /// Cheap default: ~32px triangles, 64x64 sim.
    #[default]
    Low,
    /// ~20px triangles, 96x96 sim.
    Medium,
    /// ~12px triangles, 160x160 sim.
    High,
    /// ~8px triangles, 256x256 sim (sim grid cap).
    Ultra,
}

impl WaterQuality {
    /// Every tier, low -> ultra.
    pub const ALL: [Self; 4] = [Self::Low, Self::Medium, Self::High, Self::Ultra];

    /// Parse a scene/script tier name. Case-insensitive, few aliases.
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.trim();
        for tier in Self::ALL {
            if name.eq_ignore_ascii_case(tier.name()) {
                return Some(tier);
            }
        }
        if name.eq_ignore_ascii_case("lowest") || name.eq_ignore_ascii_case("fast") {
            return Some(Self::Low);
        }
        if name.eq_ignore_ascii_case("mid") || name.eq_ignore_ascii_case("med") {
            return Some(Self::Medium);
        }
        if name.eq_ignore_ascii_case("max") || name.eq_ignore_ascii_case("highest") {
            return Some(Self::Ultra);
        }
        None
    }

    /// Canonical lowercase tier name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Ultra => "ultra",
        }
    }

    /// Target surface triangle edge length, in render-target pixels.
    ///
    /// Drives per-chunk tessellation. Smaller = denser mesh = more vertex +
    /// raster setup cost.
    pub const fn target_edge_pixels(self) -> f32 {
        match self {
            Self::Low => 32.0,
            Self::Medium => 20.0,
            Self::High => 12.0,
            Self::Ultra => 8.0,
        }
    }

    /// GPU wave sim grid per axis. Caps at the 256 cell-grid limit.
    pub const fn sim_resolution(self) -> [u32; 2] {
        match self {
            Self::Low => [64, 64],
            Self::Medium => [96, 96],
            Self::High => [160, 160],
            Self::Ultra => [256, 256],
        }
    }

    /// Default physics sample readback rate, Hz. Authors may still override
    /// `sample_readback_rate` per body.
    pub const fn sample_readback_rate(self) -> f32 {
        match self {
            Self::Low => 10.0,
            Self::Medium => 20.0,
            Self::High => 30.0,
            Self::Ultra => 60.0,
        }
    }

    /// Max surface quads per chunk axis. Always a power of 2: chunk LOD ratios
    /// stay integer, which is what keeps neighbour edges crack-free.
    pub const fn max_chunk_quads(self) -> u32 {
        match self {
            Self::Low => 16,
            Self::Medium => 32,
            Self::High => 64,
            Self::Ultra => 64,
        }
    }
}

impl fmt::Display for WaterQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_low_tier() {
        assert_eq!(WaterQuality::default(), WaterQuality::Low);
        assert_eq!(WaterQuality::default().target_edge_pixels(), 32.0);
        assert_eq!(WaterQuality::default().sim_resolution(), [64, 64]);
    }

    #[test]
    fn tiers_go_monotonically_finer() {
        let mut prev_px = f32::INFINITY;
        let mut prev_sim = 0;
        let mut prev_rate = 0.0;
        for tier in WaterQuality::ALL {
            assert!(tier.target_edge_pixels() < prev_px, "{tier}");
            assert!(tier.sim_resolution()[0] > prev_sim, "{tier}");
            assert!(tier.sample_readback_rate() > prev_rate, "{tier}");
            assert!(tier.sim_resolution()[0] <= 256);
            prev_px = tier.target_edge_pixels();
            prev_sim = tier.sim_resolution()[0];
            prev_rate = tier.sample_readback_rate();
        }
    }

    #[test]
    fn max_chunk_quads_stay_power_of_two() {
        for tier in WaterQuality::ALL {
            let q = tier.max_chunk_quads();
            assert!(q.is_power_of_two(), "{tier} -> {q}");
        }
    }

    #[test]
    fn names_round_trip_case_insensitive() {
        for tier in WaterQuality::ALL {
            assert_eq!(WaterQuality::from_name(tier.name()), Some(tier));
            assert_eq!(
                WaterQuality::from_name(&tier.name().to_ascii_uppercase()),
                Some(tier)
            );
            assert_eq!(WaterQuality::from_name(&format!("  {tier}  ")), Some(tier));
        }
        assert_eq!(WaterQuality::from_name("med"), Some(WaterQuality::Medium));
        assert_eq!(WaterQuality::from_name("max"), Some(WaterQuality::Ultra));
        assert_eq!(WaterQuality::from_name("potato"), None);
    }
}
