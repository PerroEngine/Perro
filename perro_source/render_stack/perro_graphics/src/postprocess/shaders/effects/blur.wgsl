fn blur_sample(uv: vec2<f32>, strength: f32) -> vec4<f32> {
    let s = max(strength, 0.0);
    if s <= 0.001 {
        return textureSample(input_tex, input_sampler, uv);
    }
    let o = s * post.inv_resolution;
    var sum = vec4<f32>(0.0);
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(-o.x, -o.y));
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(0.0, -o.y));
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(o.x, -o.y));
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(-o.x, 0.0));
    sum += textureSample(input_tex, input_sampler, uv);
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(o.x, 0.0));
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(-o.x, o.y));
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(0.0, o.y));
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(o.x, o.y));
    return sum / 9.0;
}
