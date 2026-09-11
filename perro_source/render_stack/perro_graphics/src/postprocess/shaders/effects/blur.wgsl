// Separable gaussian blur. One axis per pass (params0.y: 0 = horizontal,
// 1 = vertical). Two chained passes give a full 2D gaussian at ~half the
// per-pixel taps of the old non-separable 5-tap cross, with real weights.
// Note: output differs slightly from the legacy cross (accepted quality gain).
fn blur_axis_sample(uv: vec2<f32>, strength: f32, axis: f32) -> vec4<f32> {
    let s = max(strength, 0.0);
    if s <= 0.001 {
        return textureSample(input_tex, input_sampler, uv);
    }
    // params1.xy carries source texel size for scaled intermediate targets.
    // Zero keeps non-scaled callers on post.inv_resolution.
    let source_inv = select(post.inv_resolution, post.params1.xy, post.params1.x > 0.0);
    var dir = vec2<f32>(source_inv.x, 0.0);
    if axis >= 0.5 {
        dir = vec2<f32>(0.0, source_inv.y);
    }
    let step = dir * s;
    if s > 1.0 {
        // 5-tap gaussian: weights 6/16 center, 4/16, 1/16.
        var sum = textureSample(input_tex, input_sampler, uv) * 0.375;
        sum += textureSample(input_tex, input_sampler, uv - step) * 0.25;
        sum += textureSample(input_tex, input_sampler, uv + step) * 0.25;
        sum += textureSample(input_tex, input_sampler, uv - step * 2.0) * 0.0625;
        sum += textureSample(input_tex, input_sampler, uv + step * 2.0) * 0.0625;
        return sum;
    }
    // 3-tap low-strength path cuts texture reads for light menu blur.
    var sum = textureSample(input_tex, input_sampler, uv) * 0.5;
    sum += textureSample(input_tex, input_sampler, uv - step) * 0.25;
    sum += textureSample(input_tex, input_sampler, uv + step) * 0.25;
    return sum;
}
