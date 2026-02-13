pub const MESH_INSTANCED_WGSL: &str = include_str!("shaders/mesh_instanced.wgsl");

#[inline]
pub fn create_mesh_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_mesh_instanced"),
        source: wgpu::ShaderSource::Wgsl(MESH_INSTANCED_WGSL.into()),
    })
}
