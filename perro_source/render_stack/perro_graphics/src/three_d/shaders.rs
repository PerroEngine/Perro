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
    pub const SKY3D_ATMO_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/sky3d_parts/atmo.wgsl");
    pub const SKY3D_MOON_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/sky3d_parts/moon.wgsl");
    pub const SKY3D_SUN_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/sky3d_parts/sun.wgsl");
    pub const SKY3D_CLOUDS_WGSL: &str =
        perro_macros::include_str_stripped!("shaders/sky3d_parts/clouds.wgsl");
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
) -> String {
    build_material_shader_with_prelude_inner(
        prelude_wgsl,
        material_wgsl,
        std::env::var_os("PERRO_DISABLE_CUSTOM_MATERIAL_SHADOWS").is_none(),
    )
}

#[inline]
fn build_material_shader_with_prelude_inner(
    prelude_wgsl: &str,
    material_wgsl: &str,
    apply_custom_shadows: bool,
) -> String {
    let has_custom_vertex = material_wgsl.contains("shade_vertex(");
    let mut out = String::new();
    let sanitized_prelude = sanitize_reserved_meta_identifier(prelude_wgsl);
    out.push_str(&sanitized_prelude);
    out.push('\n');
    out.push_str(material_wgsl);
    if has_custom_vertex {
        out.push_str(
            "\n@vertex\nfn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {\n    return shade_vertex(perro_vs_main_base(v, inst));\n}\n",
        );
    } else {
        out.push_str(
            "\n@vertex\nfn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {\n    return perro_vs_main_base(v, inst);\n}\n",
        );
    }
    if apply_custom_shadows {
        out.push_str(
            "\nfn perro_custom_shadow_factor(in: FragmentInput) -> f32 {\n    let material = decode_material_params(in.packed_material_params);\n    if !material.receive_shadows {\n        return 1.0;\n    }\n    let n = normalize(in.normal_ws);\n    var factor = ray_shadow_factor(in.world_pos, n, vec3<f32>(0.0, 1.0, 0.0));\n    let point_count = u32(scene.ambient_and_counts.y);\n    for (var i = 0u; i < point_count; i = i + 1u) {\n        let light = scene.point_lights[i];\n        let to_light = light.position_range.xyz - in.world_pos;\n        if dot(to_light, to_light) <= light.position_range.w * light.position_range.w {\n            factor = min(factor, point_shadow_factor(in.world_pos, n, i, to_light));\n        }\n    }\n    let spot_count = u32(scene.ambient_and_counts.z);\n    for (var i = 0u; i < spot_count; i = i + 1u) {\n        let light = scene.spot_lights[i];\n        let to_light = light.position_range.xyz - in.world_pos;\n        if dot(to_light, to_light) <= light.position_range.w * light.position_range.w {\n            factor = min(factor, spot_shadow_factor(in.world_pos, n, i));\n        }\n    }\n    return mix(1.0, factor, 0.65);\n}\n\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    let color = shade_material(in);\n    let shadow_vis = perro_custom_shadow_factor(in);\n    return vec4<f32>(color.rgb * shadow_vis, color.a);\n}\n",
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
    build_sky_shader_with_parts(
        regular::SKY3D_MOON_WGSL,
        regular::SKY3D_SUN_WGSL,
        regular::SKY3D_CLOUDS_WGSL,
    )
}

#[inline]
pub fn build_sky_shader_with_parts(moon_wgsl: &str, sun_wgsl: &str, clouds_wgsl: &str) -> String {
    let mut out = String::new();
    out.push_str(regular::SKY3D_ATMO_WGSL);
    out.push('\n');
    out.push_str(moon_wgsl);
    out.push('\n');
    out.push_str(sun_wgsl);
    out.push('\n');
    out.push_str(clouds_wgsl);
    out
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
    fn custom_material_shadow_wrapper_wgsl_parses() {
        let material = "fn shade_material(in: FragmentInput) -> vec4<f32> { return vec4<f32>(in.normal_ws * 0.5 + vec3<f32>(0.5), 1.0); }";
        for prelude in [regular::PRELUDE_RIGID_WGSL, regular::PRELUDE_SKINNED_WGSL] {
            let wgsl = build_custom_material_shader_with_prelude(prelude, material);
            naga::front::wgsl::parse_str(&wgsl).expect("custom shadow material wgsl parses");
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
}
