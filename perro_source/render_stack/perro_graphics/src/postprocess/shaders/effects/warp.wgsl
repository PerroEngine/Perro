fn warp_sample(uv: vec2<f32>, waves: f32, strength: f32) -> vec4<f32> {
    let w = max(waves, 0.0);
    if w <= 0.001 {
        return textureSample(input_tex, input_sampler, uv);
    }
    let offset = sin(uv.y * w * 6.2831853) * strength * post.inv_resolution.x;
    return textureSample(input_tex, input_sampler, uv + vec2<f32>(offset, 0.0));
}
