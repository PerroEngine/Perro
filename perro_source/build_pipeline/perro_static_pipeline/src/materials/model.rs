use super::*;

#[derive(Clone)]
pub(super) enum MaterialLiteral {
    Standard(StandardMaterial3D),
    Unlit(UnlitMaterial3D),
    Toon(ToonMaterial3D),
    Custom(CustomMaterialLiteral),
}

#[derive(Clone)]
pub(super) struct CustomMaterialLiteral {
    pub(super) shader_path: String,
    pub(super) params: Vec<CustomParamLiteral>,
    pub(super) images: Vec<CustomImageLiteral>,
    pub(super) lighting: CustomMaterialLighting3D,
    pub(super) surface: StandardMaterial3D,
}

#[derive(Clone)]
pub(super) struct CustomParamLiteral {
    pub(super) name: Option<String>,
    pub(super) value: CustomMaterialParamValue3D,
}

#[derive(Clone)]
pub(super) struct CustomImageLiteral {
    pub(super) name: Option<String>,
    pub(super) source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum MaterialKey {
    Standard(StandardMaterialKey),
    Unlit(UnlitMaterialKey),
    Toon(ToonMaterialKey),
    Custom(CustomMaterialKey),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct StandardMaterialKey {
    base_color_factor: [u32; 4],
    roughness_factor: u32,
    metallic_factor: u32,
    occlusion_strength: u32,
    emissive_factor: [u32; 3],
    alpha_mode: u8,
    alpha_cutoff: u32,
    double_sided: bool,
    flat_shading: bool,
    normal_scale: u32,
    base_color_texture: u32,
    metallic_roughness_texture: u32,
    normal_texture: u32,
    occlusion_texture: u32,
    emissive_texture: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct UnlitMaterialKey {
    base_color_factor: [u32; 4],
    emissive_factor: [u32; 3],
    alpha_mode: u8,
    alpha_cutoff: u32,
    double_sided: bool,
    flat_shading: bool,
    base_color_texture: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct ToonMaterialKey {
    base_color_factor: [u32; 4],
    emissive_factor: [u32; 3],
    alpha_mode: u8,
    alpha_cutoff: u32,
    double_sided: bool,
    flat_shading: bool,
    band_count: u32,
    rim_strength: u32,
    outline_width: u32,
    base_color_texture: u32,
    ramp_texture: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct CustomMaterialKey {
    shader_path: String,
    params: Vec<CustomParamKey>,
    images: Vec<CustomImageKey>,
    lighting: CustomMaterialLighting3D,
    surface: StandardMaterialKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct CustomParamKey {
    name: Option<String>,
    value: CustomParamValueKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct CustomImageKey {
    name: Option<String>,
    source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum CustomParamValueKey {
    F32(u32),
    I32(i32),
    Bool(bool),
    Vec2([u32; 2]),
    Vec3([u32; 3]),
    Vec4([u32; 4]),
}

impl From<&MaterialLiteral> for MaterialKey {
    fn from(value: &MaterialLiteral) -> Self {
        match value {
            MaterialLiteral::Standard(v) => MaterialKey::Standard(standard_material_key(v)),
            MaterialLiteral::Unlit(v) => MaterialKey::Unlit(UnlitMaterialKey {
                base_color_factor: [
                    v.base_color_factor[0].to_bits(),
                    v.base_color_factor[1].to_bits(),
                    v.base_color_factor[2].to_bits(),
                    v.base_color_factor[3].to_bits(),
                ],
                emissive_factor: [
                    v.emissive_factor[0].to_bits(),
                    v.emissive_factor[1].to_bits(),
                    v.emissive_factor[2].to_bits(),
                ],
                alpha_mode: v.alpha_mode,
                alpha_cutoff: v.alpha_cutoff.to_bits(),
                double_sided: v.double_sided,
                flat_shading: v.flat_shading,
                base_color_texture: v.base_color_texture,
            }),
            MaterialLiteral::Toon(v) => MaterialKey::Toon(ToonMaterialKey {
                base_color_factor: [
                    v.base_color_factor[0].to_bits(),
                    v.base_color_factor[1].to_bits(),
                    v.base_color_factor[2].to_bits(),
                    v.base_color_factor[3].to_bits(),
                ],
                emissive_factor: [
                    v.emissive_factor[0].to_bits(),
                    v.emissive_factor[1].to_bits(),
                    v.emissive_factor[2].to_bits(),
                ],
                alpha_mode: v.alpha_mode,
                alpha_cutoff: v.alpha_cutoff.to_bits(),
                double_sided: v.double_sided,
                flat_shading: v.flat_shading,
                band_count: v.band_count,
                rim_strength: v.rim_strength.to_bits(),
                outline_width: v.outline_width.to_bits(),
                base_color_texture: v.base_color_texture,
                ramp_texture: v.ramp_texture,
            }),
            MaterialLiteral::Custom(v) => MaterialKey::Custom(CustomMaterialKey {
                shader_path: v.shader_path.clone(),
                lighting: v.lighting,
                surface: standard_material_key(&v.surface),
                params: v
                    .params
                    .iter()
                    .map(|p| CustomParamKey {
                        name: p.name.clone(),
                        value: match &p.value {
                            CustomMaterialParamValue3D::F32(x) => {
                                CustomParamValueKey::F32(x.to_bits())
                            }
                            CustomMaterialParamValue3D::I32(x) => CustomParamValueKey::I32(*x),
                            CustomMaterialParamValue3D::Bool(x) => CustomParamValueKey::Bool(*x),
                            CustomMaterialParamValue3D::Vec2(v) => {
                                CustomParamValueKey::Vec2([v[0].to_bits(), v[1].to_bits()])
                            }
                            CustomMaterialParamValue3D::Vec3(v) => CustomParamValueKey::Vec3([
                                v[0].to_bits(),
                                v[1].to_bits(),
                                v[2].to_bits(),
                            ]),
                            CustomMaterialParamValue3D::Vec4(v) => CustomParamValueKey::Vec4([
                                v[0].to_bits(),
                                v[1].to_bits(),
                                v[2].to_bits(),
                                v[3].to_bits(),
                            ]),
                        },
                    })
                    .collect(),
                images: v
                    .images
                    .iter()
                    .map(|image| CustomImageKey {
                        name: image.name.clone(),
                        source: image.source.clone(),
                    })
                    .collect(),
            }),
        }
    }
}

pub(super) fn standard_material_key(v: &StandardMaterial3D) -> StandardMaterialKey {
    StandardMaterialKey {
        base_color_factor: [
            v.base_color_factor[0].to_bits(),
            v.base_color_factor[1].to_bits(),
            v.base_color_factor[2].to_bits(),
            v.base_color_factor[3].to_bits(),
        ],
        roughness_factor: v.roughness_factor.to_bits(),
        metallic_factor: v.metallic_factor.to_bits(),
        occlusion_strength: v.occlusion_strength.to_bits(),
        emissive_factor: [
            v.emissive_factor[0].to_bits(),
            v.emissive_factor[1].to_bits(),
            v.emissive_factor[2].to_bits(),
        ],
        alpha_mode: v.alpha_mode,
        alpha_cutoff: v.alpha_cutoff.to_bits(),
        double_sided: v.double_sided,
        flat_shading: v.flat_shading,
        normal_scale: v.normal_scale.to_bits(),
        base_color_texture: v.base_color_texture,
        metallic_roughness_texture: v.metallic_roughness_texture,
        normal_texture: v.normal_texture,
        occlusion_texture: v.occlusion_texture,
        emissive_texture: v.emissive_texture,
    }
}
