use super::*;

pub(super) fn material_literal_to_code(material: &MaterialLiteral) -> String {
    match material {
        MaterialLiteral::Standard(m) => {
            let d = StandardMaterial3D::default();
            let mut fields = Vec::<String>::new();
            push_vertex_modifiers_field(&mut fields, m.vertex_modifiers.as_ref());
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
                    "Material3D::Standard({})",
                    standard_material_struct_to_code(m)
                )
            }
        }
        MaterialLiteral::Unlit(m) => {
            let d = UnlitMaterial3D::default();
            let mut fields = Vec::<String>::new();
            push_vertex_modifiers_field(&mut fields, m.vertex_modifiers.as_ref());
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
                    "Material3D::Unlit(UnlitMaterial3D {{ base_color_factor: {}, emissive_factor: {}, alpha_mode: {}, alpha_cutoff: {}, double_sided: {}, flat_shading: {}, base_color_texture: {}, vertex_modifiers: {} }})",
                    f32x4_to_code(m.base_color_factor),
                    f32x3_to_code(m.emissive_factor),
                    m.alpha_mode,
                    f32_to_code(m.alpha_cutoff),
                    m.double_sided,
                    m.flat_shading,
                    m.base_color_texture,
                    vertex_modifiers_to_code(m.vertex_modifiers.as_ref()),
                )
            }
        }
        MaterialLiteral::Toon(m) => {
            let d = ToonMaterial3D::default();
            let mut fields = Vec::<String>::new();
            push_vertex_modifiers_field(&mut fields, m.vertex_modifiers.as_ref());
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
                    "Material3D::Toon(ToonMaterial3D {{ base_color_factor: {}, emissive_factor: {}, alpha_mode: {}, alpha_cutoff: {}, double_sided: {}, flat_shading: {}, band_count: {}, rim_strength: {}, outline_width: {}, base_color_texture: {}, ramp_texture: {}, vertex_modifiers: {} }})",
                    f32x4_to_code(m.base_color_factor),
                    f32x3_to_code(m.emissive_factor),
                    m.alpha_mode,
                    f32_to_code(m.alpha_cutoff),
                    m.double_sided,
                    m.flat_shading,
                    m.band_count,
                    f32_to_code(m.rim_strength),
                    f32_to_code(m.outline_width),
                    m.base_color_texture,
                    m.ramp_texture,
                    vertex_modifiers_to_code(m.vertex_modifiers.as_ref()),
                )
            }
        }
        MaterialLiteral::HandDrawn(m) => {
            let d = HandDrawnMaterial3D::default();
            let mut fields = common_stylized_fields(
                m.base_color_factor,
                m.emissive_factor,
                m.alpha_mode,
                m.alpha_cutoff,
                m.double_sided,
                m.flat_shading,
                m.base_color_texture,
                d.base_color_factor,
                d.emissive_factor,
                d.alpha_mode,
                d.alpha_cutoff,
                d.double_sided,
                d.flat_shading,
                d.base_color_texture,
            );
            push_vertex_modifiers_field(&mut fields, m.vertex_modifiers.as_ref());
            if m.band_count != d.band_count {
                fields.push(format!("band_count: {}", m.band_count));
            }
            if m.hatch_scale != d.hatch_scale {
                fields.push(format!("hatch_scale: {}", f32_to_code(m.hatch_scale)));
            }
            if m.grain_strength != d.grain_strength {
                fields.push(format!("grain_strength: {}", f32_to_code(m.grain_strength)));
            }
            if fields.is_empty() {
                "Material3D::HandDrawn(HandDrawnMaterial3D::const_default())".to_string()
            } else {
                format!(
                    "Material3D::HandDrawn(HandDrawnMaterial3D {{ base_color_factor: {}, emissive_factor: {}, alpha_mode: {}, alpha_cutoff: {}, double_sided: {}, flat_shading: {}, band_count: {}, hatch_scale: {}, grain_strength: {}, base_color_texture: {}, vertex_modifiers: {} }})",
                    f32x4_to_code(m.base_color_factor),
                    f32x3_to_code(m.emissive_factor),
                    m.alpha_mode,
                    f32_to_code(m.alpha_cutoff),
                    m.double_sided,
                    m.flat_shading,
                    m.band_count,
                    f32_to_code(m.hatch_scale),
                    f32_to_code(m.grain_strength),
                    m.base_color_texture,
                    vertex_modifiers_to_code(m.vertex_modifiers.as_ref()),
                )
            }
        }
        MaterialLiteral::PixelSurface(m) => {
            let d = PixelSurfaceMaterial3D::default();
            let mut fields = common_stylized_fields(
                m.base_color_factor,
                m.emissive_factor,
                m.alpha_mode,
                m.alpha_cutoff,
                m.double_sided,
                m.flat_shading,
                m.base_color_texture,
                d.base_color_factor,
                d.emissive_factor,
                d.alpha_mode,
                d.alpha_cutoff,
                d.double_sided,
                d.flat_shading,
                d.base_color_texture,
            );
            push_vertex_modifiers_field(&mut fields, m.vertex_modifiers.as_ref());
            if m.pixel_count != d.pixel_count {
                fields.push(format!("pixel_count: {}", m.pixel_count));
            }
            if m.color_levels != d.color_levels {
                fields.push(format!("color_levels: {}", m.color_levels));
            }
            if m.dither_strength != d.dither_strength {
                fields.push(format!(
                    "dither_strength: {}",
                    f32_to_code(m.dither_strength)
                ));
            }
            if fields.is_empty() {
                "Material3D::PixelSurface(PixelSurfaceMaterial3D::const_default())".to_string()
            } else {
                format!(
                    "Material3D::PixelSurface(PixelSurfaceMaterial3D {{ base_color_factor: {}, emissive_factor: {}, alpha_mode: {}, alpha_cutoff: {}, double_sided: {}, flat_shading: {}, pixel_count: {}, color_levels: {}, dither_strength: {}, base_color_texture: {}, vertex_modifiers: {} }})",
                    f32x4_to_code(m.base_color_factor),
                    f32x3_to_code(m.emissive_factor),
                    m.alpha_mode,
                    f32_to_code(m.alpha_cutoff),
                    m.double_sided,
                    m.flat_shading,
                    m.pixel_count,
                    m.color_levels,
                    f32_to_code(m.dither_strength),
                    m.base_color_texture,
                    vertex_modifiers_to_code(m.vertex_modifiers.as_ref()),
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

#[allow(clippy::too_many_arguments)]
fn common_stylized_fields(
    base_color_factor: [f32; 4],
    emissive_factor: [f32; 3],
    alpha_mode: u8,
    alpha_cutoff: f32,
    double_sided: bool,
    flat_shading: bool,
    base_color_texture: u32,
    default_base_color_factor: [f32; 4],
    default_emissive_factor: [f32; 3],
    default_alpha_mode: u8,
    default_alpha_cutoff: f32,
    default_double_sided: bool,
    default_flat_shading: bool,
    default_base_color_texture: u32,
) -> Vec<String> {
    let mut fields = Vec::new();
    if base_color_factor != default_base_color_factor {
        fields.push(format!(
            "base_color_factor: {}",
            f32x4_to_code(base_color_factor)
        ));
    }
    if emissive_factor != default_emissive_factor {
        fields.push(format!(
            "emissive_factor: {}",
            f32x3_to_code(emissive_factor)
        ));
    }
    if alpha_mode != default_alpha_mode {
        fields.push(format!("alpha_mode: {alpha_mode}"));
    }
    if alpha_cutoff != default_alpha_cutoff {
        fields.push(format!("alpha_cutoff: {}", f32_to_code(alpha_cutoff)));
    }
    if double_sided != default_double_sided {
        fields.push(format!("double_sided: {double_sided}"));
    }
    if flat_shading != default_flat_shading {
        fields.push(format!("flat_shading: {flat_shading}"));
    }
    if base_color_texture != default_base_color_texture {
        fields.push(format!("base_color_texture: {base_color_texture}"));
    }
    fields
}

fn push_vertex_modifiers_field(fields: &mut Vec<String>, modifiers: &[VertexModifier3D]) {
    if modifiers.is_empty() {
        return;
    }
    fields.push(format!(
        "vertex_modifiers: {}",
        vertex_modifiers_to_code(modifiers)
    ));
}

fn vertex_modifiers_to_code(modifiers: &[VertexModifier3D]) -> String {
    let body = modifiers
        .iter()
        .map(vertex_modifier_to_code)
        .collect::<Vec<_>>()
        .join(", ");
    format!("Cow::Borrowed(&[{body}])")
}

fn vertex_modifier_to_code(modifier: &VertexModifier3D) -> String {
    match *modifier {
        VertexModifier3D::Wind {
            direction,
            strength,
            speed,
            frequency,
            mask,
        } => format!(
            "VertexModifier3D::Wind {{ direction: {}, strength: {}, speed: {}, frequency: {}, mask: {} }}",
            f32x3_to_code(direction),
            f32_to_code(strength),
            f32_to_code(speed),
            f32_to_code(frequency),
            vertex_mask_to_code(mask),
        ),
        VertexModifier3D::Wave {
            axis,
            direction,
            amplitude,
            speed,
            frequency,
            phase,
            mask,
        } => format!(
            "VertexModifier3D::Wave {{ axis: {}, direction: {}, amplitude: {}, speed: {}, frequency: {}, phase: {}, mask: {} }}",
            vertex_axis_to_code(axis),
            f32x3_to_code(direction),
            f32_to_code(amplitude),
            f32_to_code(speed),
            f32_to_code(frequency),
            f32_to_code(phase),
            optional_vertex_mask_to_code(mask),
        ),
        VertexModifier3D::Bend {
            along_axis,
            bend_axis,
            angle_radians,
            start,
            end,
        } => format!(
            "VertexModifier3D::Bend {{ along_axis: {}, bend_axis: {}, angle_radians: {}, start: {}, end: {} }}",
            vertex_axis_to_code(along_axis),
            vertex_axis_to_code(bend_axis),
            f32_to_code(angle_radians),
            f32_to_code(start),
            f32_to_code(end),
        ),
        VertexModifier3D::Twist {
            axis,
            angle_radians,
            start,
            end,
        } => format!(
            "VertexModifier3D::Twist {{ axis: {}, angle_radians: {}, start: {}, end: {} }}",
            vertex_axis_to_code(axis),
            f32_to_code(angle_radians),
            f32_to_code(start),
            f32_to_code(end),
        ),
        VertexModifier3D::Inflate { amount, mask } => format!(
            "VertexModifier3D::Inflate {{ amount: {}, mask: {} }}",
            f32_to_code(amount),
            optional_vertex_mask_to_code(mask),
        ),
        VertexModifier3D::Jitter {
            amount,
            scale,
            rate,
            seed,
            mask,
        } => format!(
            "VertexModifier3D::Jitter {{ amount: {}, scale: {}, rate: {}, seed: {}, mask: {} }}",
            f32_to_code(amount),
            f32_to_code(scale),
            f32_to_code(rate),
            seed,
            optional_vertex_mask_to_code(mask),
        ),
        VertexModifier3D::PixelSnap {
            virtual_height,
            strength,
        } => format!(
            "VertexModifier3D::PixelSnap {{ virtual_height: {virtual_height}, strength: {} }}",
            f32_to_code(strength),
        ),
    }
}

fn vertex_axis_to_code(axis: VertexAxis3D) -> &'static str {
    match axis {
        VertexAxis3D::X => "VertexAxis3D::X",
        VertexAxis3D::Y => "VertexAxis3D::Y",
        VertexAxis3D::Z => "VertexAxis3D::Z",
    }
}

fn vertex_mask_to_code(mask: VertexModifierMask3D) -> String {
    format!(
        "VertexModifierMask3D {{ axis: {}, start: {}, end: {} }}",
        vertex_axis_to_code(mask.axis),
        f32_to_code(mask.start),
        f32_to_code(mask.end),
    )
}

fn optional_vertex_mask_to_code(mask: Option<VertexModifierMask3D>) -> String {
    match mask {
        Some(mask) => format!("Some({})", vertex_mask_to_code(mask)),
        None => "None".to_string(),
    }
}

pub(super) fn standard_material_struct_to_code(m: &StandardMaterial3D) -> String {
    let d = StandardMaterial3D::default();
    let mut fields = Vec::<String>::new();
    push_vertex_modifiers_field(&mut fields, m.vertex_modifiers.as_ref());
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
            "StandardMaterial3D {{ base_color_factor: {}, roughness_factor: {}, metallic_factor: {}, occlusion_strength: {}, emissive_factor: {}, alpha_mode: {}, alpha_cutoff: {}, double_sided: {}, flat_shading: {}, normal_scale: {}, base_color_texture: {}, metallic_roughness_texture: {}, normal_texture: {}, occlusion_texture: {}, emissive_texture: {}, vertex_modifiers: {} }}",
            f32x4_to_code(m.base_color_factor),
            f32_to_code(m.roughness_factor),
            f32_to_code(m.metallic_factor),
            f32_to_code(m.occlusion_strength),
            f32x3_to_code(m.emissive_factor),
            m.alpha_mode,
            f32_to_code(m.alpha_cutoff),
            m.double_sided,
            m.flat_shading,
            f32_to_code(m.normal_scale),
            m.base_color_texture,
            m.metallic_roughness_texture,
            m.normal_texture,
            m.occlusion_texture,
            m.emissive_texture,
            vertex_modifiers_to_code(m.vertex_modifiers.as_ref()),
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
