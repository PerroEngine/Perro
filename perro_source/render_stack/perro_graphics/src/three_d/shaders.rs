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
pub fn build_material_shader_with_prelude(prelude_wgsl: &str, material_wgsl: &str) -> String {
    let has_custom_vertex = material_wgsl.contains("shade_vertex(");
    let mut out = String::new();
    out.push_str(prelude_wgsl);
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
    out.push_str(
        "\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    return shade_material(in);\n}\n",
    );
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
fn build_sky_shader() -> String {
    let mut out = String::new();
    out.push_str(regular::SKY3D_ATMO_WGSL);
    out.push('\n');
    out.push_str(regular::SKY3D_MOON_WGSL);
    out.push('\n');
    out.push_str(regular::SKY3D_SUN_WGSL);
    out.push('\n');
    out.push_str(regular::SKY3D_CLOUDS_WGSL);
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
