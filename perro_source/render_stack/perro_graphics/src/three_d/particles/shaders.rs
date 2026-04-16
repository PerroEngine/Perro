pub const POINT_PARTICLES_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/point_particles.wgsl");
pub const POINT_PARTICLES_GPU_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/point_particles_gpu.wgsl");
pub const POINT_PARTICLES_COMPUTE_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/point_particles_compute.wgsl");

#[inline]
pub fn create_point_particles_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_point_particles"),
        source: wgpu::ShaderSource::Wgsl(POINT_PARTICLES_WGSL.into()),
    })
}

#[inline]
pub fn create_point_particles_gpu_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_point_particles_gpu"),
        source: wgpu::ShaderSource::Wgsl(POINT_PARTICLES_GPU_WGSL.into()),
    })
}

#[inline]
pub fn create_point_particles_compute_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_point_particles_compute"),
        source: wgpu::ShaderSource::Wgsl(POINT_PARTICLES_COMPUTE_WGSL.into()),
    })
}
