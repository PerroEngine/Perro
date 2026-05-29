mod regular {
    pub const PRELUDE_WGSL: &str = perro_macros::include_str_stripped!("shaders/prelude_3d.wgsl");
    pub const PRELUDE_RIGID_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/prelude_rigid_3d.wgsl");
    pub const PRELUDE_SKINNED_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/prelude_skinned_3d.wgsl");
    pub const MATERIAL_STANDARD_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/material_standard.wgsl");
    pub const MATERIAL_UNLIT_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/material_unlit.wgsl");
    pub const MATERIAL_TOON_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/material_toon.wgsl");
    pub const DEPTH_PREPASS_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/depth_prepass.wgsl");
    pub const DEPTH_PREPASS_RIGID_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/depth_prepass_rigid.wgsl");
    pub const DEPTH_PREPASS_SKINNED_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/depth_prepass_skinned.wgsl");
    pub const MULTIMESH_WGSL: &str = perro_macros::include_str_stripped!("shaders/multimesh.wgsl");
    pub const SKY3D_WGSL: &str = perro_macros::include_str_stripped!("shaders/sky3d.wgsl");
}

mod culling {
    pub const FRUSTUM_CULL_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/frustum_cull.wgsl");
    pub const HIZ_DEPTH_COPY_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/hiz_depth_copy.wgsl");
    pub const HIZ_DOWNSAMPLE_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/hiz_downsample.wgsl");
    pub const HIZ_OCCLUSION_CULL_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/hiz_occlusion_cull.wgsl");
}

#[inline]
pub fn create_mesh_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_instanced"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader(regular::MATERIAL_STANDARD_WGSL).into(),
        ),
    })
}

#[inline]
pub fn create_mesh_shader_module_rigid(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_instanced_rigid"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                regular::PRELUDE_RIGID_WGSL,
                regular::MATERIAL_STANDARD_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_unlit_shader_module_rigid(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_unlit_rigid"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                regular::PRELUDE_RIGID_WGSL,
                regular::MATERIAL_UNLIT_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_toon_shader_module_rigid(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_toon_rigid"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                regular::PRELUDE_RIGID_WGSL,
                regular::MATERIAL_TOON_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_mesh_shader_module_skinned(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_instanced_skinned"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                regular::PRELUDE_SKINNED_WGSL,
                regular::MATERIAL_STANDARD_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_unlit_shader_module_skinned(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_unlit_skinned"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                regular::PRELUDE_SKINNED_WGSL,
                regular::MATERIAL_UNLIT_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_toon_shader_module_skinned(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_toon_skinned"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader_with_prelude(
                regular::PRELUDE_SKINNED_WGSL,
                regular::MATERIAL_TOON_WGSL,
            )
            .into(),
        ),
    })
}

#[inline]
pub fn create_unlit_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_unlit"),
        source: wgpu::ShaderSource::Wgsl(
            build_material_shader(regular::MATERIAL_UNLIT_WGSL).into(),
        ),
    })
}

#[inline]
pub fn create_toon_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_toon"),
        source: wgpu::ShaderSource::Wgsl(build_material_shader(regular::MATERIAL_TOON_WGSL).into()),
    })
}

#[inline]
pub fn build_material_shader(material_wgsl: &str) -> String {
    build_material_shader_with_prelude(regular::PRELUDE_WGSL, material_wgsl)
}

#[inline]
fn sanitize_reserved_meta_identifier(wgsl: &str) -> String {
    wgsl.replace(
        "let meta = custom_params_meta",
        "let packed_meta = custom_params_meta",
    )
    .replace(
        "let meta = blend_shape_instances",
        "let blend_meta = blend_shape_instances",
    )
    .replace("let kind = meta & 0x3u;", "let kind = packed_meta & 0x3u;")
    .replace(
        "let value_offset = meta >> 2u;",
        "let value_offset = packed_meta >> 2u;",
    )
}

#[inline]
pub fn build_material_shader_with_prelude(prelude_wgsl: &str, material_wgsl: &str) -> String {
    build_material_shader_with_prelude_inner(prelude_wgsl, material_wgsl, false)
}

#[inline]
pub fn build_custom_material_shader_with_prelude(
    prelude_wgsl: &str,
    material_wgsl: &str,
    lighting: perro_render_bridge::CustomMaterialLighting3D,
) -> String {
    let uses_lit_helper = material_wgsl.contains("perro_lit_standard(");
    let apply_standard_lighting =
        lighting == perro_render_bridge::CustomMaterialLighting3D::Standard && !uses_lit_helper;
    build_material_shader_with_prelude_inner(prelude_wgsl, material_wgsl, apply_standard_lighting)
}

#[inline]
fn build_material_shader_with_prelude_inner(
    prelude_wgsl: &str,
    material_wgsl: &str,
    apply_custom_standard_lighting: bool,
) -> String {
    let has_custom_vertex = material_wgsl.contains("shade_vertex(");
    let mut out = String::new();
    let sanitized_prelude = sanitize_reserved_meta_identifier(prelude_wgsl);
    out.push_str(&sanitized_prelude);
    out.push('\n');
    out.push_str(material_wgsl);
    if has_custom_vertex {
        out.push_str(
            "\n@vertex\nfn vs_main(v: VertexInput, inst: InstanceInput, @builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput {\n    return shade_vertex(perro_vs_main_base(v, inst, vertex_index, instance_index));\n}\n",
        );
    } else {
        out.push_str(
            "\n@vertex\nfn vs_main(v: VertexInput, inst: InstanceInput, @builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput {\n    return perro_vs_main_base(v, inst, vertex_index, instance_index);\n}\n",
        );
    }
    if apply_custom_standard_lighting {
        out.push_str(
            "\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    let base = shade_material(in);\n    return perro_lit_standard(in, base, 0.5, 0.0, 1.0, vec3<f32>(0.0));\n}\n",
        );
    } else {
        out.push_str(
            "\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    return shade_material(in);\n}\n",
        );
    }
    out
}

#[inline]
pub fn create_depth_prepass_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_depth_prepass"),
        source: wgpu::ShaderSource::Wgsl(regular::DEPTH_PREPASS_WGSL.into()),
    })
}

#[inline]
pub fn create_depth_prepass_shader_module_rigid(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_depth_prepass_rigid"),
        source: wgpu::ShaderSource::Wgsl(regular::DEPTH_PREPASS_RIGID_WGSL.into()),
    })
}

#[inline]
pub fn create_depth_prepass_shader_module_skinned(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_depth_prepass_skinned"),
        source: wgpu::ShaderSource::Wgsl(regular::DEPTH_PREPASS_SKINNED_WGSL.into()),
    })
}

#[inline]
pub fn create_multimesh_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_multimesh"),
        source: wgpu::ShaderSource::Wgsl(regular::MULTIMESH_WGSL.into()),
    })
}

#[inline]
pub fn create_sky_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_sky3d"),
        source: wgpu::ShaderSource::Wgsl(build_sky_shader().into()),
    })
}

#[inline]
pub fn create_sky_shader_module_from_source(
    device: &wgpu::Device,
    source: String,
) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_sky3d_custom"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}

#[inline]
pub fn build_sky_shader() -> String {
    regular::SKY3D_WGSL.replace(
        "/*__PERRO_SKY_CUSTOM_STACK__*/",
        "fn apply_custom_sky_stack(base: SkyFragment) -> vec4<f32> { return base.color; }",
    )
}

#[inline]
pub fn build_sky_shader_with_passes(
    passes: &[(String, &[perro_structs::CustomPostParam])],
) -> String {
    let mut stack = String::new();
    for (idx, (source, params)) in passes.iter().enumerate() {
        let fn_name = format!("sky_shader_{idx}");
        let renamed = source.replacen("fn sky_shader", &format!("fn {fn_name}"), 1);
        stack.push('\n');
        stack.push_str(&renamed);
        stack.push('\n');
        stack.push_str(&format!(
            "fn apply_sky_shader_pass_{idx}(base: SkyFragment) -> vec4<f32> {{\n"
        ));
        stack.push_str("    let frag = SkyFragment(\n");
        stack.push_str("        base.ray,\n");
        stack.push_str("        base.uv,\n");
        stack.push_str("        base.time_of_day,\n");
        stack.push_str("        base.time_seconds,\n");
        stack.push_str("        base.day_weight,\n");
        stack.push_str("        base.evening_weight,\n");
        stack.push_str("        base.night_weight,\n");
        stack.push_str("        base.horizon_weight,\n");
        stack.push_str("        base.color,\n");
        stack.push_str(&encoded_sky_param_values(params));
        stack.push_str("    );\n");
        stack.push_str(&format!("    return {fn_name}(frag);\n"));
        stack.push_str("}\n");
        stack.push_str(&format!(
            "fn sky_custom_pass_{idx}(base: SkyFragment) -> vec4<f32> {{ return apply_sky_shader_pass_{idx}(base); }}\n"
        ));
    }
    if !passes.is_empty() {
        stack.push_str("\nfn apply_custom_sky_stack(base: SkyFragment) -> vec4<f32> {\n");
        stack.push_str("    var cur = base;\n");
        for idx in 0..passes.len() {
            stack.push_str(&format!("    cur.color = sky_custom_pass_{idx}(cur);\n"));
        }
        stack.push_str("    return cur.color;\n");
        stack.push_str("}\n");
    }
    regular::SKY3D_WGSL.replace("/*__PERRO_SKY_CUSTOM_STACK__*/", &stack)
}

fn encoded_sky_param_values(params: &[perro_structs::CustomPostParam]) -> String {
    let mut out = String::new();
    for i in 0..16 {
        let v = params
            .get(i)
            .map(|param| encode_custom_param_value(&param.value))
            .unwrap_or([0.0; 4]);
        out.push_str(&format!(
            "        vec4<f32>({x}, {y}, {z}, {w}),\n",
            x = wgsl_f32(v[0]),
            y = wgsl_f32(v[1]),
            z = wgsl_f32(v[2]),
            w = wgsl_f32(v[3])
        ));
    }
    out
}

fn encode_custom_param_value(value: &perro_structs::CustomPostParamValue) -> [f32; 4] {
    match value {
        perro_structs::CustomPostParamValue::F32(v) => [*v, 0.0, 0.0, 0.0],
        perro_structs::CustomPostParamValue::I32(v) => [*v as f32, 0.0, 0.0, 0.0],
        perro_structs::CustomPostParamValue::Bool(v) => [if *v { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        perro_structs::CustomPostParamValue::Vec2(v) => [v[0], v[1], 0.0, 0.0],
        perro_structs::CustomPostParamValue::Vec3(v) => [v[0], v[1], v[2], 0.0],
        perro_structs::CustomPostParamValue::Vec4(v) => *v,
    }
}

fn wgsl_f32(v: f32) -> String {
    if v.is_finite() {
        format!("{v:?}")
    } else {
        "0.0".to_string()
    }
}

#[inline]
pub fn create_frustum_cull_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_frustum_cull"),
        source: wgpu::ShaderSource::Wgsl(culling::FRUSTUM_CULL_WGSL.into()),
    })
}

#[inline]
pub fn create_hiz_depth_copy_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_hiz_depth_copy"),
        source: wgpu::ShaderSource::Wgsl(culling::HIZ_DEPTH_COPY_WGSL.into()),
    })
}

#[inline]
pub fn create_hiz_downsample_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_hiz_downsample"),
        source: wgpu::ShaderSource::Wgsl(culling::HIZ_DOWNSAMPLE_WGSL.into()),
    })
}

#[inline]
pub fn create_hiz_occlusion_cull_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_hiz_occlusion_cull"),
        source: wgpu::ShaderSource::Wgsl(culling::HIZ_OCCLUSION_CULL_WGSL.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use naga::valid::{Capabilities, ValidationFlags, Validator};

    fn parse_and_validate(wgsl: &str, label: &str) {
        let module =
            naga::front::wgsl::parse_str(wgsl).unwrap_or_else(|err| panic!("{label}: {err}"));
        Validator::new(ValidationFlags::all(), Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|err| panic!("{label}: {err}"));
    }

    #[test]
    fn three_d_material_wgsl_parses() {
        for prelude in [
            regular::PRELUDE_WGSL,
            regular::PRELUDE_RIGID_WGSL,
            regular::PRELUDE_SKINNED_WGSL,
        ] {
            for material in [
                regular::MATERIAL_STANDARD_WGSL,
                regular::MATERIAL_UNLIT_WGSL,
                regular::MATERIAL_TOON_WGSL,
            ] {
                let wgsl = build_material_shader_with_prelude(prelude, material);
                naga::front::wgsl::parse_str(&wgsl).expect("3d material wgsl parses");
            }
        }
    }

    #[test]
    fn custom_material_standard_lighting_wrapper_wgsl_parses() {
        let material = "fn shade_material(in: FragmentInput) -> vec4<f32> { return vec4<f32>(in.normal_ws * 0.5 + vec3<f32>(0.5), 1.0); }";
        for prelude in [regular::PRELUDE_RIGID_WGSL, regular::PRELUDE_SKINNED_WGSL] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Standard,
            );
            assert!(wgsl.contains("perro_lit_standard(in, base"));
            naga::front::wgsl::parse_str(&wgsl).expect("custom lit wrapper material wgsl parses");
        }
    }

    #[test]
    fn custom_material_raw_wrapper_wgsl_parses() {
        let material = "fn shade_material(in: FragmentInput) -> vec4<f32> { return vec4<f32>(in.normal_ws * 0.5 + vec3<f32>(0.5), 1.0); }";
        for prelude in [regular::PRELUDE_RIGID_WGSL, regular::PRELUDE_SKINNED_WGSL] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Raw,
            );
            assert!(!wgsl.contains("perro_lit_standard(in, base"));
            naga::front::wgsl::parse_str(&wgsl).expect("custom raw material wgsl parses");
        }
    }

    #[test]
    fn custom_material_lit_helper_wgsl_parses() {
        let material = r#"
fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let emissive = unpack_rgba8(in.packed_emissive).xyz;
    let pbr = decode_standard_pbr_params(in.packed_pbr_params_0, in.packed_pbr_params_1);
    return perro_lit_standard(in, color, pbr.x, pbr.y, pbr.z, emissive);
}
"#;
        for prelude in [regular::PRELUDE_RIGID_WGSL, regular::PRELUDE_SKINNED_WGSL] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Standard,
            );
            assert!(!wgsl.contains("perro_lit_standard(in, base"));
            naga::front::wgsl::parse_str(&wgsl).expect("custom lit material wgsl parses");
        }
    }

    #[test]
    fn custom_material_shade_vertex_wgsl_validates() {
        let material = r#"
fn shade_vertex(out: VertexOutput) -> VertexOutput {
    let wobble = custom_v_param(out, 0u).x;
    var next = out;
    next.world_pos.y = next.world_pos.y + wobble;
    next.clip_pos.y = next.clip_pos.y + wobble;
    return next;
}

fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    return vec4<f32>(color.rgb, perro_material_alpha(in, color.a));
}
"#;
        for prelude in [
            regular::PRELUDE_WGSL,
            regular::PRELUDE_RIGID_WGSL,
            regular::PRELUDE_SKINNED_WGSL,
        ] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Raw,
            );
            assert!(wgsl.contains("return shade_vertex(perro_vs_main_base"));
            parse_and_validate(&wgsl, "custom shade_vertex material wgsl validates");
        }
    }

    #[test]
    fn custom_material_shader_interface_has_no_meshlet_inputs() {
        let material = r#"
fn shade_vertex(out: VertexOutput) -> VertexOutput {
    var next = out;
    next.world_pos.x = next.world_pos.x + custom_v_param(out, 0u).x;
    return next;
}

fn shade_material(in: FragmentInput) -> vec4<f32> {
    return vec4<f32>(custom_f_param(in, 0u).xyz + in.uv.xyx, 1.0);
}
"#;
        let vertex_entry = "fn vs_main(v: VertexInput, inst: InstanceInput, @builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput";
        let fragment_entry = "fn fs_main(in: FragmentInput) -> @location(0) vec4<f32>";
        for prelude in [
            regular::PRELUDE_WGSL,
            regular::PRELUDE_RIGID_WGSL,
            regular::PRELUDE_SKINNED_WGSL,
        ] {
            let wgsl = build_custom_material_shader_with_prelude(
                prelude,
                material,
                perro_render_bridge::CustomMaterialLighting3D::Raw,
            );
            assert!(wgsl.contains(vertex_entry));
            assert!(wgsl.contains(fragment_entry));
            assert!(wgsl.contains("return shade_vertex(perro_vs_main_base"));
            assert!(wgsl.contains("return shade_material(in);"));
            assert!(!wgsl.contains("@location(3) meshlet"));
            assert!(!wgsl.contains("meshlet_index"));
            parse_and_validate(&wgsl, "custom shader interface stays meshlet-free");
        }
    }

    #[test]
    fn multimesh_wgsl_parses() {
        naga::front::wgsl::parse_str(regular::MULTIMESH_WGSL).expect("multimesh wgsl parses");
    }

    #[test]
    fn sky_wgsl_parses() {
        let wgsl = build_sky_shader();
        naga::front::wgsl::parse_str(&wgsl).expect("sky wgsl parses");
    }

    #[test]
    fn custom_sky_wgsl_parses() {
        let custom = r#"
fn sky_shader(in: SkyFragment) -> vec4<f32> {
    return vec4<f32>(in.color.rgb + custom_param(in, 0u).xxx, in.color.a);
}
"#
        .to_string();
        let params = vec![perro_structs::CustomPostParam::unnamed(
            perro_structs::CustomPostParamValue::F32(0.1),
        )];
        let wgsl = build_sky_shader_with_passes(&[(custom, params.as_slice())]);
        naga::front::wgsl::parse_str(&wgsl).expect("custom sky wgsl parses");
    }
}
