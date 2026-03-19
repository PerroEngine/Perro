fn pixelate_sample(uv: vec2<f32>, size: f32) -> vec4<f32> {
    let px = max(size, 1.0);
    let pix = floor(uv * post.resolution / px) * px / post.resolution;
    return textureSample(input_tex, input_sampler, pix);
}
