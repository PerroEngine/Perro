pub(super) const UI_SHADER: &str = r#"
struct UiUniform {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> ui: UiUniform;
@group(1) @binding(0) var font_tex: texture_2d<f32>;
@group(1) @binding(1) var font_sampler: sampler;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let x = (in.pos.x / max(ui.screen_size.x, 1.0)) * 2.0 - 1.0;
    let y = 1.0 - (in.pos.y / max(ui.screen_size.y, 1.0)) * 2.0;
    var out: VsOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

fn linear_from_gamma_rgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

@fragment
fn fs_main_linear_framebuffer(in: VsOut) -> @location(0) vec4<f32> {
    let sample = textureSample(font_tex, font_sampler, in.uv);
    let color = vec4<f32>(linear_from_gamma_rgb(in.color.rgb), in.color.a);
    return sample * color;
}
"#;

pub(super) const UI_COMPOSITE_SHADER: &str = r#"
@group(0) @binding(0) var composite_tex: texture_2d<f32>;
@group(0) @binding(1) var composite_sampler: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_composite(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    let pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vertex_index], 0.0, 1.0);
    out.uv = uv[vertex_index];
    return out;
}

@fragment
fn fs_composite_gamma_framebuffer(in: VsOut) -> @location(0) vec4<f32> {
    let linear = textureSample(composite_tex, composite_sampler, in.uv);
    let cutoff = linear.rgb <= vec3<f32>(0.0031308);
    let lower = linear.rgb * vec3<f32>(12.92);
    let higher = vec3<f32>(1.055) * pow(linear.rgb, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return vec4<f32>(select(higher, lower, cutoff), linear.a);
}

@fragment
fn fs_composite_linear_framebuffer(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(composite_tex, composite_sampler, in.uv);
}
"#;

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
    fn ui_shaders_validate() {
        parse_and_validate(UI_SHADER, "ui shader");
        parse_and_validate(UI_COMPOSITE_SHADER, "ui composite shader");
    }
}
