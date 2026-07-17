use super::*;

pub(super) fn material_literal_to_code(material: &MaterialLiteral) -> String {
    match material {
        MaterialLiteral::Standard(m) => {
            let d = StandardMaterial3D::default();
            let mut fields = Vec::<String>::new();
            if m.base_color_factor != d.base_color_factor {
                fields.push(format!(
                    "base_color_factor: {}",
                    f32x4_to_code(m.base_color_factor)
                ));
            }
            if m.roughness_factor != d.roughness_factor {
                fields.push(format!(
                    "roughness_factor: {}",
                    f32_to_code(m.roughness_factor)
                ));
            }
            if m.metallic_factor != d.metallic_factor {
                fields.push(format!(
                    "metallic_factor: {}",
                    f32_to_code(m.metallic_factor)
                ));
            }
            if m.occlusion_strength != d.occlusion_strength {
                fields.push(format!(
                    "occlusion_strength: {}",
                    f32_to_code(m.occlusion_strength)
                ));
            }
            if m.emissive_factor != d.emissive_factor {
                fields.push(format!(
                    "emissive_factor: {}",
                    f32x3_to_code(m.emissive_factor)
                ));
            }
            if m.alpha_mode != d.alpha_mode {
                fields.push(format!("alpha_mode: {}", m.alpha_mode));
            }
            if m.alpha_cutoff != d.alpha_cutoff {
                fields.push(format!("alpha_cutoff: {}", f32_to_code(m.alpha_cutoff)));
            }
            if m.double_sided != d.double_sided {
                fields.push(format!(
                    "double_sided: {}",
                    if m.double_sided { "true" } else { "false" }
                ));
            }
            if m.flat_shading != d.flat_shading {
                fields.push(format!(
                    "flat_shading: {}",
                    if m.flat_shading { "true" } else { "false" }
                ));
            }
            if m.normal_scale != d.normal_scale {
                fields.push(format!("normal_scale: {}", f32_to_code(m.normal_scale)));
            }
            if m.base_color_texture != d.base_color_texture {
                fields.push(format!("base_color_texture: {}", m.base_color_texture));
            }
            if m.metallic_roughness_texture != d.metallic_roughness_texture {
                fields.push(format!(
                    "metallic_roughness_texture: {}",
                    m.metallic_roughness_texture
                ));
            }
            if m.normal_texture != d.normal_texture {
                fields.push(format!("normal_texture: {}", m.normal_texture));
            }
            if m.occlusion_texture != d.occlusion_texture {
                fields.push(format!("occlusion_texture: {}", m.occlusion_texture));
            }
            if m.emissive_texture != d.emissive_texture {
                fields.push(format!("emissive_texture: {}", m.emissive_texture));
            }
            if fields.is_empty() {
                "Material3D::Standard(StandardMaterial3D::const_default())".to_string()
            } else {
                format!(
                    "Material3D::Standard(StandardMaterial3D {{ {}, ..StandardMaterial3D::const_default() }})",
                    fields.join(", ")
                )
            }
        }
        MaterialLiteral::Unlit(m) => {
            let d = UnlitMaterial3D::default();
            let mut fields = Vec::<String>::new();
            if m.base_color_factor != d.base_color_factor {
                fields.push(format!(
                    "base_color_factor: {}",
                    f32x4_to_code(m.base_color_factor)
                ));
            }
            if m.emissive_factor != d.emissive_factor {
                fields.push(format!(
                    "emissive_factor: {}",
                    f32x3_to_code(m.emissive_factor)
                ));
            }
            if m.alpha_mode != d.alpha_mode {
                fields.push(format!("alpha_mode: {}", m.alpha_mode));
            }
            if m.alpha_cutoff != d.alpha_cutoff {
                fields.push(format!("alpha_cutoff: {}", f32_to_code(m.alpha_cutoff)));
            }
            if m.double_sided != d.double_sided {
                fields.push(format!(
                    "double_sided: {}",
                    if m.double_sided { "true" } else { "false" }
                ));
            }
            if m.flat_shading != d.flat_shading {
                fields.push(format!(
                    "flat_shading: {}",
                    if m.flat_shading { "true" } else { "false" }
                ));
            }
            if m.base_color_texture != d.base_color_texture {
                fields.push(format!("base_color_texture: {}", m.base_color_texture));
            }
            if fields.is_empty() {
                "Material3D::Unlit(UnlitMaterial3D::const_default())".to_string()
            } else {
                format!(
                    "Material3D::Unlit(UnlitMaterial3D {{ {}, ..UnlitMaterial3D::const_default() }})",
                    fields.join(", ")
                )
            }
        }
        MaterialLiteral::Toon(m) => {
            let d = ToonMaterial3D::default();
            let mut fields = Vec::<String>::new();
            if m.base_color_factor != d.base_color_factor {
                fields.push(format!(
                    "base_color_factor: {}",
                    f32x4_to_code(m.base_color_factor)
                ));
            }
            if m.emissive_factor != d.emissive_factor {
                fields.push(format!(
                    "emissive_factor: {}",
                    f32x3_to_code(m.emissive_factor)
                ));
            }
            if m.alpha_mode != d.alpha_mode {
                fields.push(format!("alpha_mode: {}", m.alpha_mode));
            }
            if m.alpha_cutoff != d.alpha_cutoff {
                fields.push(format!("alpha_cutoff: {}", f32_to_code(m.alpha_cutoff)));
            }
            if m.double_sided != d.double_sided {
                fields.push(format!(
                    "double_sided: {}",
                    if m.double_sided { "true" } else { "false" }
                ));
            }
            if m.flat_shading != d.flat_shading {
                fields.push(format!(
                    "flat_shading: {}",
                    if m.flat_shading { "true" } else { "false" }
                ));
            }
            if m.band_count != d.band_count {
                fields.push(format!("band_count: {}", m.band_count));
            }
            if m.rim_strength != d.rim_strength {
                fields.push(format!("rim_strength: {}", f32_to_code(m.rim_strength)));
            }
            if m.outline_width != d.outline_width {
                fields.push(format!("outline_width: {}", f32_to_code(m.outline_width)));
            }
            if m.base_color_texture != d.base_color_texture {
                fields.push(format!("base_color_texture: {}", m.base_color_texture));
            }
            if m.ramp_texture != d.ramp_texture {
                fields.push(format!("ramp_texture: {}", m.ramp_texture));
            }
            if fields.is_empty() {
                "Material3D::Toon(ToonMaterial3D::const_default())".to_string()
            } else {
                format!(
                    "Material3D::Toon(ToonMaterial3D {{ {}, ..ToonMaterial3D::const_default() }})",
                    fields.join(", ")
                )
            }
        }
        MaterialLiteral::Custom(m) => {
            let params = if m.params.is_empty() {
                "Cow::Borrowed(&[])".to_string()
            } else {
                let mut rendered = String::from("Cow::Borrowed(&[");
                for (i, param) in m.params.iter().enumerate() {
                    if i > 0 {
                        rendered.push_str(", ");
                    }
                    rendered.push_str("CustomMaterialParam3D { name: ");
                    match &param.name {
                        Some(name) => {
                            rendered.push_str(&format!("Some(Cow::Borrowed({:?}))", name));
                        }
                        None => rendered.push_str("None"),
                    }
                    rendered.push_str(", value: ");
                    rendered.push_str(&custom_param_value_to_code(&param.value));
                    rendered.push_str(" }");
                }
                rendered.push_str("])");
                rendered
            };
            let images = if m.images.is_empty() {
                "Cow::Borrowed(&[])".to_string()
            } else {
                let mut rendered = String::from("Cow::Borrowed(&[");
                for (i, image) in m.images.iter().enumerate() {
                    if i > 0 {
                        rendered.push_str(", ");
                    }
                    rendered.push_str("CustomMaterialImage3D { name: ");
                    match &image.name {
                        Some(name) => {
                            rendered.push_str(&format!("Some(Cow::Borrowed({:?}))", name));
                        }
                        None => rendered.push_str("None"),
                    }
                    rendered.push_str(", source: ");
                    rendered.push_str(&format!("Cow::Borrowed({:?})", image.source));
                    rendered.push_str(" }");
                }
                rendered.push_str("])");
                rendered
            };
            format!(
                "Material3D::Custom(CustomMaterial3D {{ shader_path: Cow::Borrowed({:?}), params: {}, images: {}, lighting: {}, surface: {} }})",
                m.shader_path,
                params,
                images,
                custom_lighting_to_code(m.lighting),
                standard_material_struct_to_code(&m.surface)
            )
        }
    }
}

pub(super) fn standard_material_struct_to_code(m: &StandardMaterial3D) -> String {
    let d = StandardMaterial3D::default();
    let mut fields = Vec::<String>::new();
    if m.base_color_factor != d.base_color_factor {
        fields.push(format!(
            "base_color_factor: {}",
            f32x4_to_code(m.base_color_factor)
        ));
    }
    if m.roughness_factor != d.roughness_factor {
        fields.push(format!(
            "roughness_factor: {}",
            f32_to_code(m.roughness_factor)
        ));
    }
    if m.metallic_factor != d.metallic_factor {
        fields.push(format!(
            "metallic_factor: {}",
            f32_to_code(m.metallic_factor)
        ));
    }
    if m.occlusion_strength != d.occlusion_strength {
        fields.push(format!(
            "occlusion_strength: {}",
            f32_to_code(m.occlusion_strength)
        ));
    }
    if m.emissive_factor != d.emissive_factor {
        fields.push(format!(
            "emissive_factor: {}",
            f32x3_to_code(m.emissive_factor)
        ));
    }
    if m.alpha_mode != d.alpha_mode {
        fields.push(format!("alpha_mode: {}", m.alpha_mode));
    }
    if m.alpha_cutoff != d.alpha_cutoff {
        fields.push(format!("alpha_cutoff: {}", f32_to_code(m.alpha_cutoff)));
    }
    if m.double_sided != d.double_sided {
        fields.push(format!(
            "double_sided: {}",
            if m.double_sided { "true" } else { "false" }
        ));
    }
    if m.flat_shading != d.flat_shading {
        fields.push(format!(
            "flat_shading: {}",
            if m.flat_shading { "true" } else { "false" }
        ));
    }
    if m.normal_scale != d.normal_scale {
        fields.push(format!("normal_scale: {}", f32_to_code(m.normal_scale)));
    }
    if m.base_color_texture != d.base_color_texture {
        fields.push(format!("base_color_texture: {}", m.base_color_texture));
    }
    if m.metallic_roughness_texture != d.metallic_roughness_texture {
        fields.push(format!(
            "metallic_roughness_texture: {}",
            m.metallic_roughness_texture
        ));
    }
    if m.normal_texture != d.normal_texture {
        fields.push(format!("normal_texture: {}", m.normal_texture));
    }
    if m.occlusion_texture != d.occlusion_texture {
        fields.push(format!("occlusion_texture: {}", m.occlusion_texture));
    }
    if m.emissive_texture != d.emissive_texture {
        fields.push(format!("emissive_texture: {}", m.emissive_texture));
    }
    if fields.is_empty() {
        "StandardMaterial3D::const_default()".to_string()
    } else {
        format!(
            "StandardMaterial3D {{ {}, ..StandardMaterial3D::const_default() }}",
            fields.join(", ")
        )
    }
}

pub(super) fn custom_lighting_to_code(lighting: CustomMaterialLighting3D) -> &'static str {
    match lighting {
        CustomMaterialLighting3D::Standard => "CustomMaterialLighting3D::Standard",
        CustomMaterialLighting3D::Raw => "CustomMaterialLighting3D::Raw",
    }
}

pub(super) fn custom_param_value_to_code(value: &CustomMaterialParamValue3D) -> String {
    match value {
        CustomMaterialParamValue3D::F32(v) => {
            format!("CustomMaterialParamValue3D::F32({})", f32_to_code(*v))
        }
        CustomMaterialParamValue3D::I32(v) => format!("CustomMaterialParamValue3D::I32({})", v),
        CustomMaterialParamValue3D::Bool(v) => format!(
            "CustomMaterialParamValue3D::Bool({})",
            if *v { "true" } else { "false" }
        ),
        CustomMaterialParamValue3D::Vec2(v) => {
            format!("CustomMaterialParamValue3D::Vec2({})", f32x2_to_code(*v))
        }
        CustomMaterialParamValue3D::Vec3(v) => {
            format!("CustomMaterialParamValue3D::Vec3({})", f32x3_to_code(*v))
        }
        CustomMaterialParamValue3D::Vec4(v) => {
            format!("CustomMaterialParamValue3D::Vec4({})", f32x4_to_code(*v))
        }
    }
}

pub(super) fn f32_to_code(value: f32) -> String {
    format!("f32::from_bits({:#010x})", value.to_bits())
}

pub(super) fn f32x2_to_code(value: [f32; 2]) -> String {
    format!("[{}, {}]", f32_to_code(value[0]), f32_to_code(value[1]))
}

pub(super) fn f32x3_to_code(value: [f32; 3]) -> String {
    format!(
        "[{}, {}, {}]",
        f32_to_code(value[0]),
        f32_to_code(value[1]),
        f32_to_code(value[2])
    )
}

pub(super) fn f32x4_to_code(value: [f32; 4]) -> String {
    format!(
        "[{}, {}, {}, {}]",
        f32_to_code(value[0]),
        f32_to_code(value[1]),
        f32_to_code(value[2]),
        f32_to_code(value[3])
    )
}
