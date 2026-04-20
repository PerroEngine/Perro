pub const POINT_PARTICLES_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/point_particles.wgsl");
pub const POINT_PARTICLES_GPU_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/point_particles_gpu.wgsl");
pub const POINT_PARTICLES_COMPUTE_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/point_particles_compute.wgsl");
pub const POINT_PARTICLES_COMPUTE_STUB_WGSL: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) emissive: vec3<f32>,
    @location(2) alpha: f32,
}

@compute @workgroup_size(64)
fn cs_main() {}

@vertex
fn vs_main(@builtin(instance_index) _instance: u32) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(2.0, 2.0, 0.0, 1.0);
    out.color = vec4<f32>(0.0);
    out.emissive = vec3<f32>(0.0);
    out.alpha = 0.0;
    return out;
}

@vertex
fn vs_billboard(
    @builtin(vertex_index) _vertex_index: u32,
    @builtin(instance_index) _instance: u32,
) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(2.0, 2.0, 0.0, 1.0);
    out.color = vec4<f32>(0.0);
    out.emissive = vec3<f32>(0.0);
    out.alpha = 0.0;
    return out;
}

@fragment
fn fs_main(
    @location(0) _color: vec4<f32>,
    @location(1) _emissive: vec3<f32>,
    @location(2) _alpha: f32,
) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}
"#;

#[inline]
fn gpu_compute_particles_enabled() -> bool {
    std::env::var("PERRO_ENABLE_GPU_COMPUTE_PARTICLES")
        .ok()
        .as_deref()
        .map(|v| matches!(v, "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false)
}

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
    let source = if gpu_compute_particles_enabled() {
        POINT_PARTICLES_COMPUTE_WGSL
    } else {
        POINT_PARTICLES_COMPUTE_STUB_WGSL
    };
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_point_particles_compute"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}
