const BUILTIN_POST_BODY_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/postprocess_builtin_body.wgsl");
const EFFECT_BLUR_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/blur.wgsl");
const EFFECT_PIXELATE_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/effects/pixelate.wgsl");
const EFFECT_PIXEL_ART_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/effects/pixel_art.wgsl");
const EFFECT_WARP_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/warp.wgsl");
const EFFECT_VIGNETTE_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/effects/vignette.wgsl");
const EFFECT_CRT_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/crt.wgsl");
const EFFECT_COLOR_FILTER_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/effects/color_filter.wgsl");
const EFFECT_REVERSE_FILTER_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/effects/reverse_filter.wgsl");
const EFFECT_CHROMA_KEY_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/effects/chroma_key.wgsl");
const EFFECT_BLOOM_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/bloom.wgsl");
const EFFECT_SATURATE_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/effects/saturate.wgsl");
const EFFECT_BLACK_WHITE_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/effects/black_white.wgsl");
const EFFECT_COLOR_GRADE_WGSL: &str =
    perro_macros::include_str_stripped!("shaders/effects/color_grade.wgsl");
const EFFECT_LUT_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/lut.wgsl");

pub fn create_builtin_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    let wgsl = build_builtin_post_shader();
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_post_builtin"),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    })
}

fn build_builtin_post_shader() -> String {
    let mut wgsl = String::new();
    wgsl.push_str(PRELUDE_WGSL);
    wgsl.push_str(EFFECT_BLUR_WGSL);
    wgsl.push_str(EFFECT_PIXELATE_WGSL);
    wgsl.push_str(EFFECT_PIXEL_ART_WGSL);
    wgsl.push_str(EFFECT_WARP_WGSL);
    wgsl.push_str(EFFECT_VIGNETTE_WGSL);
    wgsl.push_str(EFFECT_CRT_WGSL);
    wgsl.push_str(EFFECT_COLOR_FILTER_WGSL);
    wgsl.push_str(EFFECT_REVERSE_FILTER_WGSL);
    wgsl.push_str(EFFECT_CHROMA_KEY_WGSL);
    wgsl.push_str(EFFECT_BLOOM_WGSL);
    wgsl.push_str(EFFECT_SATURATE_WGSL);
    wgsl.push_str(EFFECT_BLACK_WHITE_WGSL);
    wgsl.push_str(EFFECT_COLOR_GRADE_WGSL);
    wgsl.push_str(EFFECT_LUT_WGSL);
    wgsl.push_str(BUILTIN_POST_BODY_WGSL);
    wgsl
}

// Composition lives in perro_wgsl so `perro doctor` and the static build can
// check a game's post-process pass without linking wgpu.
pub use perro_wgsl::compose::build_post_shader;

const PRELUDE_WGSL: &str = perro_wgsl::compose::POST_PRELUDE_WGSL;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_post_wgsl_parses() {
        let wgsl = build_builtin_post_shader();
        let module = naga::front::wgsl::parse_str(&wgsl).expect("builtin post wgsl parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("builtin post wgsl validates");
    }
}
