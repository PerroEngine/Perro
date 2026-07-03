// Bloom as a downsample -> blur -> upsample composite chain. Bright-pass and
// blur passes run on a half-res target (~4x less fill), then composite adds the
// upsampled bloom back over the full-res scene.

// Bright-pass: extract luminance above threshold. Rendered into the half-res
// target; the linear sampler downsamples the full-res input for free.
fn bloom_bright_sample(uv: vec2<f32>, threshold: f32) -> vec4<f32> {
    let base = textureSample(input_tex, input_sampler, uv);
    let t = clamp(threshold, 0.0, 1.0);
    let luma = dot(base.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let bright = base.rgb * smoothstep(t, 1.0, luma);
    return vec4<f32>(bright, 1.0);
}

// Composite: add the blurred half-res bloom (bound in the lut_2d slot) over the
// full-res scene in input_tex. Linear sampling upsamples the half-res bloom.
fn bloom_composite_sample(uv: vec2<f32>, strength: f32) -> vec4<f32> {
    let base = textureSample(input_tex, input_sampler, uv);
    let bloom = textureSample(lut_2d_tex, input_sampler, uv).rgb;
    let s = max(strength, 0.0);
    return vec4<f32>(base.rgb + bloom * s, base.a);
}
