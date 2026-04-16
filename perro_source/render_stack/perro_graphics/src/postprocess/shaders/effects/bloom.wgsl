fn bloom_sample(uv: vec2<f32>, strength: f32, threshold: f32, radius: f32) -> vec4<f32> {
    let base = textureSample(input_tex, input_sampler, uv);
    let t = clamp(threshold, 0.0, 1.0);
    let s = max(strength, 0.0);
    let r = max(radius, 0.0);
    if s <= 0.001 || r <= 0.001 {
        return base;
    }
    let o = r * post.inv_resolution;
    var sum = vec3<f32>(0.0);
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(-o.x, -o.y)).rgb;
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(0.0, -o.y)).rgb;
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(o.x, -o.y)).rgb;
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(-o.x, 0.0)).rgb;
    sum += base.rgb;
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(o.x, 0.0)).rgb;
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(-o.x, o.y)).rgb;
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(0.0, o.y)).rgb;
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(o.x, o.y)).rgb;
    let blur = sum / 9.0;
    let luma = dot(blur, vec3<f32>(0.2126, 0.7152, 0.0722));
    let bloom = blur * smoothstep(t, 1.0, luma);
    let out_rgb = base.rgb + bloom * s;
    return vec4<f32>(out_rgb, base.a);
}
