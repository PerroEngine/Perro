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

#[cfg(test)]
mod wgsl_validation_tests {
    use super::*;

    fn parse_and_validate(wgsl: &str, label: &str) {
        let module =
            naga::front::wgsl::parse_str(wgsl).unwrap_or_else(|err| panic!("{label}: {err}"));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|err| panic!("{label}: {err}"));
    }

    #[test]
    fn two_d_shaders_validate() {
        parse_and_validate(SPRITE_INSTANCED_WGSL, "sprite instanced");
        parse_and_validate(RECT_INSTANCED_WGSL, "rect instanced");
        parse_and_validate(POINT_LIGHT_2D_WGSL, "point light 2d");
    }
}
