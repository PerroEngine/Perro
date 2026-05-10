pub const SPRITE_INSTANCED_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/sprite_instanced.wgsl");
pub const RECT_INSTANCED_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/rect_instanced.wgsl");
pub const POINT_LIGHT_2D_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/point_light_2d.wgsl");

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

#[inline]
pub fn create_point_light_2d_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_point_light_2d"),
        source: wgpu::ShaderSource::Wgsl(POINT_LIGHT_2D_WGSL.into()),
    })
}
