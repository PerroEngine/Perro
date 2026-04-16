const ACCESSIBILITY_SHADER_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/accessibility.wgsl");

pub fn create_accessibility_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_visual_accessibility_shader"),
        source: wgpu::ShaderSource::Wgsl(ACCESSIBILITY_SHADER_WGSL.into()),
    })
}
