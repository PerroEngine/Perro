pub const POINT_PARTICLES_WGSL: &str = include_str!("shaders/point_particles.wgsl");

#[inline]
pub fn create_point_particles_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_point_particles"),
        source: wgpu::ShaderSource::Wgsl(POINT_PARTICLES_WGSL.into()),
    })
}
