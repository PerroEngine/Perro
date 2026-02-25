pub const MESH_INSTANCED_WGSL: &str = include_str!("shaders/mesh_instanced.wgsl");
pub const DEPTH_PREPASS_WGSL: &str = include_str!("shaders/depth_prepass.wgsl");
pub const FRUSTUM_CULL_WGSL: &str = include_str!("shaders/frustum_cull.wgsl");
pub const HIZ_DEPTH_COPY_WGSL: &str = include_str!("shaders/hiz_depth_copy.wgsl");
pub const HIZ_DOWNSAMPLE_WGSL: &str = include_str!("shaders/hiz_downsample.wgsl");
pub const HIZ_OCCLUSION_CULL_WGSL: &str = include_str!("shaders/hiz_occlusion_cull.wgsl");

#[inline]
pub fn create_mesh_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_instanced"),
        source: wgpu::ShaderSource::Wgsl(MESH_INSTANCED_WGSL.into()),
    })
}

#[inline]
pub fn create_depth_prepass_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_depth_prepass"),
        source: wgpu::ShaderSource::Wgsl(DEPTH_PREPASS_WGSL.into()),
    })
}

#[inline]
pub fn create_frustum_cull_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_frustum_cull"),
        source: wgpu::ShaderSource::Wgsl(FRUSTUM_CULL_WGSL.into()),
    })
}

#[inline]
pub fn create_hiz_depth_copy_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_hiz_depth_copy"),
        source: wgpu::ShaderSource::Wgsl(HIZ_DEPTH_COPY_WGSL.into()),
    })
}

#[inline]
pub fn create_hiz_downsample_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_hiz_downsample"),
        source: wgpu::ShaderSource::Wgsl(HIZ_DOWNSAMPLE_WGSL.into()),
    })
}

#[inline]
pub fn create_hiz_occlusion_cull_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_hiz_occlusion_cull"),
        source: wgpu::ShaderSource::Wgsl(HIZ_OCCLUSION_CULL_WGSL.into()),
    })
}
