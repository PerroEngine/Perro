use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;

use crate::renderer_3d::MaterialUniform;

/// Represents a physically‑based material similar to Blender’s Principled BSDF
/// and glTF 2.0’s PBR metallic‑roughness model.
///
/// This is the CPU‑side / asset‑level description. In the renderer, it will
/// later be converted to a GPU‑friendly `MaterialUniform`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    /// Base surface color (albedo).  
    /// Equivalent to “Base Color” in Blender’s Principled BSDF.
    pub base_color: [f32; 3],

    /// Metallic value: 0.0 = dielectric (non‑metal), 1.0 = pure metal.
    pub metallic: f32,

    /// Roughness: 0.0 = smooth/mirror, 1.0 = rough/diffuse.
    pub roughness: f32,

    /// Emissive color (self‑illumination).
    pub emissive: [f32; 3],

    /// Optional base‑color (albedo) texture resource path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub albedo_texture_path: Option<Cow<'static, str>>,

    /// Optional normal map texture path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normal_texture_path: Option<Cow<'static, str>>,

    /// Optional combined metallic‑roughness texture path  
    /// (roughness → G channel, metallic → B channel).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metallic_roughness_texture_path: Option<Cow<'static, str>>,

    /// Optional display name (used by editor or inspector).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Cow<'static, str>>,
}

impl fmt::Display for Material {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Material(base_color:[{},{},{}], metallic:{}, roughness:{})",
            self.base_color[0],
            self.base_color[1],
            self.base_color[2],
            self.metallic,
            self.roughness
        )
    }
}

impl Material {
    pub fn to_uniform(&self) -> MaterialUniform {
        MaterialUniform {
            base_color: [
                self.base_color[0],
                self.base_color[1],
                self.base_color[2],
                1.0,
            ],
            metallic: self.metallic,
            roughness: self.roughness,
            _pad0: [0.0; 2],
            emissive: [self.emissive[0], self.emissive[1], self.emissive[2], 0.0],
        }
    }
}
