// Bloom as a downsample -> blur -> upsample composite chain. Bright-pass and
// blur passes run on a half-res target (~4x less fill), then composite adds the
// upsampled bloom back over the full-res scene.

// Bright-pass in scene-referred linear light. Threshold may exceed 1.0.
fn bloom_bright_sample(uv: vec2<f32>, threshold: f32) -> vec4<f32> {
    let base = textureSample(input_tex, input_sampler, uv);
    let t = max(threshold, 0.0);
    let luma = dot(base.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let knee = max(t * 0.25, 0.0001);
    let bright = base.rgb * smoothstep(t - knee, t + knee, luma);
    return vec4<f32>(bright, 1.0);
}

// Fuse bright extraction, downsample, and horizontal blur. Sampling the
// full-res source at half-res texel offsets matches the old bright target's
// footprint while removing one half-res render pass + texture round trip.
fn bloom_bright_blur_sample(uv: vec2<f32>, threshold: f32, radius: f32) -> vec4<f32> {
    let r = max(radius, 0.0);
    if r <= 0.001 {
        return bloom_bright_sample(uv, threshold);
    }
    let step = vec2<f32>(post.inv_resolution.x * r, 0.0);
    var sum = bloom_bright_sample(uv, threshold) * 0.375;
    sum += bloom_bright_sample(uv - step, threshold) * 0.25;
    sum += bloom_bright_sample(uv + step, threshold) * 0.25;
    sum += bloom_bright_sample(uv - step * 2.0, threshold) * 0.0625;
    sum += bloom_bright_sample(uv + step * 2.0, threshold) * 0.0625;
    return sum;
}

// Composite: add the blurred half-res bloom (bound in the lut_2d slot) over the
// full-res scene in input_tex. Linear sampling upsamples the half-res bloom.
fn bloom_composite_sample(uv: vec2<f32>, strength: f32) -> vec4<f32> {
    let base = textureSample(input_tex, input_sampler, uv);
    let bloom = textureSample(lut_2d_tex, input_sampler, uv).rgb;
    let s = max(strength, 0.0);
    return vec4<f32>(base.rgb + bloom * s, base.a);
}
