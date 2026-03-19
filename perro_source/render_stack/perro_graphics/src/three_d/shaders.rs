mod regular {
    pub const PRELUDE_WGSL: &str = include_str!("shaders/prelude_3d.wgsl");
    pub const MATERIAL_STANDARD_WGSL: &str = include_str!("shaders/material_standard.wgsl");
    pub const MATERIAL_UNLIT_WGSL: &str = include_str!("shaders/material_unlit.wgsl");
    pub const MATERIAL_TOON_WGSL: &str = include_str!("shaders/material_toon.wgsl");
    pub const DEPTH_PREPASS_WGSL: &str = include_str!("shaders/depth_prepass.wgsl");
}

mod culling {
    pub const FRUSTUM_CULL_WGSL: &str = include_str!("shaders/frustum_cull.wgsl");
    pub const HIZ_DEPTH_COPY_WGSL: &str = include_str!("shaders/hiz_depth_copy.wgsl");
    pub const HIZ_DOWNSAMPLE_WGSL: &str = include_str!("shaders/hiz_downsample.wgsl");
    pub const HIZ_OCCLUSION_CULL_WGSL: &str = include_str!("shaders/hiz_occlusion_cull.wgsl");
}

#[inline]
pub fn create_mesh_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_instanced"),
        source: wgpu::ShaderSource::Wgsl(build_material_shader(regular::MATERIAL_STANDARD_WGSL)),
    })
}

#[inline]
pub fn create_unlit_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_unlit"),
        source: wgpu::ShaderSource::Wgsl(build_material_shader(regular::MATERIAL_UNLIT_WGSL)),
    })
}

#[inline]
pub fn create_toon_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_toon"),
        source: wgpu::ShaderSource::Wgsl(build_material_shader(regular::MATERIAL_TOON_WGSL)),
    })
}

#[inline]
pub fn build_material_shader(material_wgsl: &str) -> String {
    let mut out = String::new();
    out.push_str(regular::PRELUDE_WGSL);
    out.push('\n');
    out.push_str(material_wgsl);
    out.push_str("\n@fragment\nfn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {\n    return shade_material(in);\n}\n");
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
