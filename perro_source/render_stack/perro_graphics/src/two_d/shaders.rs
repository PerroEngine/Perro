pub const SPRITE_INSTANCED_WGSL: &str = include_str!("shaders/sprite_instanced.wgsl");
pub const RECT_INSTANCED_WGSL: &str = include_str!("shaders/rect_instanced.wgsl");

#[inline]
pub fn create_sprite_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_sprite_instanced"),
        source: wgpu::ShaderSource::Wgsl(SPRITE_INSTANCED_WGSL.into()),
    })
}

#[inline]
pub fn create_rect_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_rect_instanced"),
        source: wgpu::ShaderSource::Wgsl(RECT_INSTANCED_WGSL.into()),
    })
}
