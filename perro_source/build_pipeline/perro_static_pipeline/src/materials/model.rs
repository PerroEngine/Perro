use super::*;

#[derive(Clone)]
pub(super) enum MaterialLiteral {
    Standard(StandardMaterial3D),
    Unlit(UnlitMaterial3D),
    Toon(ToonMaterial3D),
    HandDrawn(HandDrawnMaterial3D),
    PixelSurface(PixelSurfaceMaterial3D),
    Custom(CustomMaterialLiteral),
}

#[derive(Clone)]
pub(super) struct CustomMaterialLiteral {
    pub(super) shader_path: String,
    pub(super) params: Vec<CustomParamLiteral>,
    pub(super) images: Vec<CustomImageLiteral>,
    pub(super) lighting: CustomMaterialLighting3D,
    pub(super) surface: StandardMaterial3D,
    pub(crate) release_bake: bool,
    pub(crate) bake_resolution: Option<[u32; 2]>,
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
    HandDrawn(HandDrawnMaterialKey),
    PixelSurface(PixelSurfaceMaterialKey),
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
    vertex_modifiers: Vec<u32>,
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
    vertex_modifiers: Vec<u32>,
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
    vertex_modifiers: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct HandDrawnMaterialKey {
    base_color_factor: [u32; 4],
    emissive_factor: [u32; 3],
    alpha_mode: u8,
    alpha_cutoff: u32,
    double_sided: bool,
    flat_shading: bool,
    band_count: u32,
    hatch_scale: u32,
    grain_strength: u32,
    base_color_texture: u32,
    vertex_modifiers: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct PixelSurfaceMaterialKey {
    base_color_factor: [u32; 4],
    emissive_factor: [u32; 3],
    alpha_mode: u8,
    alpha_cutoff: u32,
    double_sided: bool,
    flat_shading: bool,
    pixel_count: u32,
    color_levels: u32,
    dither_strength: u32,
    base_color_texture: u32,
    vertex_modifiers: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct CustomMaterialKey {
    shader_path: String,
    params: Vec<CustomParamKey>,
    images: Vec<CustomImageKey>,
    lighting: CustomMaterialLighting3D,
    surface: StandardMaterialKey,
    release_bake: bool,
    bake_resolution: Option<[u32; 2]>,
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
                vertex_modifiers: vertex_modifier_key(v.vertex_modifiers.as_ref()),
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
                vertex_modifiers: vertex_modifier_key(v.vertex_modifiers.as_ref()),
            }),
            MaterialLiteral::HandDrawn(v) => MaterialKey::HandDrawn(HandDrawnMaterialKey {
                base_color_factor: v.base_color_factor.map(f32::to_bits),
                emissive_factor: v.emissive_factor.map(f32::to_bits),
                alpha_mode: v.alpha_mode,
                alpha_cutoff: v.alpha_cutoff.to_bits(),
                double_sided: v.double_sided,
                flat_shading: v.flat_shading,
                band_count: v.band_count,
                hatch_scale: v.hatch_scale.to_bits(),
                grain_strength: v.grain_strength.to_bits(),
                base_color_texture: v.base_color_texture,
                vertex_modifiers: vertex_modifier_key(v.vertex_modifiers.as_ref()),
            }),
            MaterialLiteral::PixelSurface(v) => {
                MaterialKey::PixelSurface(PixelSurfaceMaterialKey {
                    base_color_factor: v.base_color_factor.map(f32::to_bits),
                    emissive_factor: v.emissive_factor.map(f32::to_bits),
                    alpha_mode: v.alpha_mode,
                    alpha_cutoff: v.alpha_cutoff.to_bits(),
                    double_sided: v.double_sided,
                    flat_shading: v.flat_shading,
                    pixel_count: v.pixel_count,
                    color_levels: v.color_levels,
                    dither_strength: v.dither_strength.to_bits(),
                    base_color_texture: v.base_color_texture,
                    vertex_modifiers: vertex_modifier_key(v.vertex_modifiers.as_ref()),
                })
            }
            MaterialLiteral::Custom(v) => MaterialKey::Custom(CustomMaterialKey {
                shader_path: v.shader_path.clone(),
                lighting: v.lighting,
                surface: standard_material_key(&v.surface),
                release_bake: v.release_bake,
                bake_resolution: v.bake_resolution,
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
        vertex_modifiers: vertex_modifier_key(v.vertex_modifiers.as_ref()),
    }
}

fn vertex_modifier_key(modifiers: &[VertexModifier3D]) -> Vec<u32> {
    let mut out = Vec::new();
    for modifier in modifiers {
        match *modifier {
            VertexModifier3D::Wind {
                direction,
                strength,
                speed,
                frequency,
                mask,
            } => {
                out.extend([
                    0,
                    direction[0].to_bits(),
                    direction[1].to_bits(),
                    direction[2].to_bits(),
                ]);
                out.extend([strength.to_bits(), speed.to_bits(), frequency.to_bits()]);
                push_mask_key(&mut out, Some(mask));
            }
            VertexModifier3D::Wave {
                axis,
                direction,
                amplitude,
                speed,
                frequency,
                phase,
                mask,
            } => {
                out.extend([
                    1,
                    axis_key(axis),
                    direction[0].to_bits(),
                    direction[1].to_bits(),
                    direction[2].to_bits(),
                ]);
                out.extend([
                    amplitude.to_bits(),
                    speed.to_bits(),
                    frequency.to_bits(),
                    phase.to_bits(),
                ]);
                push_mask_key(&mut out, mask);
            }
            VertexModifier3D::Bend {
                along_axis,
                bend_axis,
                angle_radians,
                start,
                end,
            } => {
                out.extend([
                    2,
                    axis_key(along_axis),
                    axis_key(bend_axis),
                    angle_radians.to_bits(),
                    start.to_bits(),
                    end.to_bits(),
                ]);
            }
            VertexModifier3D::Twist {
                axis,
                angle_radians,
                start,
                end,
            } => {
                out.extend([
                    3,
                    axis_key(axis),
                    angle_radians.to_bits(),
                    start.to_bits(),
                    end.to_bits(),
                ]);
            }
            VertexModifier3D::Inflate { amount, mask } => {
                out.extend([4, amount.to_bits()]);
                push_mask_key(&mut out, mask);
            }
            VertexModifier3D::Jitter {
                amount,
                scale,
                rate,
                seed,
                mask,
            } => {
                out.extend([5, amount.to_bits(), scale.to_bits(), rate.to_bits(), seed]);
                push_mask_key(&mut out, mask);
            }
            VertexModifier3D::PixelSnap {
                virtual_height,
                strength,
            } => {
                out.extend([6, virtual_height, strength.to_bits()]);
            }
        }
    }
    out
}

fn push_mask_key(out: &mut Vec<u32>, mask: Option<VertexModifierMask3D>) {
    match mask {
        Some(mask) => out.extend([
            1,
            axis_key(mask.axis),
            mask.start.to_bits(),
            mask.end.to_bits(),
        ]),
        None => out.extend([0, 0, 0, 0]),
    }
}

fn axis_key(axis: VertexAxis3D) -> u32 {
    match axis {
        VertexAxis3D::X => 0,
        VertexAxis3D::Y => 1,
        VertexAxis3D::Z => 2,
    }
}
