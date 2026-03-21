struct AccessibilityUniform {
    mode: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    params0: vec4<f32>,
};

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> accessibility: AccessibilityUniform;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vid], 0.0, 1.0);
    out.uv = (out.pos.xy * vec2<f32>(0.5, -0.5)) + vec2<f32>(0.5, 0.5);
    return out;
}

fn color_blind_sim_matrix(mode: u32) -> mat3x3<f32> {
    if mode == 0u {
        // Protan deficiency simulation
        return mat3x3<f32>(
            vec3<f32>(0.567, 0.433, 0.000),
            vec3<f32>(0.558, 0.442, 0.000),
            vec3<f32>(0.000, 0.242, 0.758),
        );
    }
    if mode == 1u {
        // Deuteran deficiency simulation
        return mat3x3<f32>(
            vec3<f32>(0.625, 0.375, 0.000),
            vec3<f32>(0.700, 0.300, 0.000),
            vec3<f32>(0.000, 0.300, 0.700),
        );
    }
    // Tritan deficiency simulation
    return mat3x3<f32>(
        vec3<f32>(0.950, 0.050, 0.000),
        vec3<f32>(0.000, 0.433, 0.567),
        vec3<f32>(0.000, 0.475, 0.525),
    );
}

fn daltonize_correct(mode: u32, rgb: vec3<f32>) -> vec3<f32> {
    let sim = color_blind_sim_matrix(mode) * rgb;
    let err = rgb - sim;

    var corr = vec3<f32>(0.0, 0.0, 0.0);
    if mode == 0u {
        // Protan correction
        corr = vec3<f32>(
            0.0,
            0.7 * err.x + err.y,
            0.7 * err.x + err.z,
        );
    } else if mode == 1u {
        // Deuteran correction
        corr = vec3<f32>(
            err.x + 0.7 * err.y,
            0.0,
            err.z + 0.7 * err.y,
        );
    } else {
        // Tritan correction
        corr = vec3<f32>(
            err.x + 0.7 * err.z,
            err.y + 0.7 * err.z,
            0.0,
        );
    }
    return clamp(rgb + corr, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn achroma_grayscale(rgb: vec3<f32>, strength: f32) -> vec3<f32> {
    // Wavelength-biased grayscale:
    // red -> darker, green -> mid, blue -> brighter influence.
    let spectral = dot(rgb, vec3<f32>(0.06, 0.26, 0.68));

    // Preserve scene lighting atmosphere by anchoring to standard luminance.
    let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let gray = mix(luma, spectral, 0.88);

    // Keep a mild contrast boost without crushing the mood.
    let c = 1.0 + 0.22 * clamp(strength, 0.0, 1.0);
    let out_luma = clamp((gray - 0.5) * c + 0.5, 0.0, 1.0);
    return vec3<f32>(out_luma, out_luma, out_luma);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let color = textureSample(input_tex, input_sampler, in.uv);
    let t = clamp(accessibility.params0.x, 0.0, 1.0);
    var corrected: vec3<f32>;
    if accessibility.mode == 3u {
        corrected = achroma_grayscale(color.rgb, t);
    } else {
        corrected = daltonize_correct(accessibility.mode, color.rgb);
    }
    return vec4<f32>(mix(color.rgb, corrected, t), color.a);
}
