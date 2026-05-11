fn lut_cube_size_2d(requested: f32) -> f32 {
    if requested >= 2.0 {
        return requested;
    }
    let dims = textureDimensions(lut_2d_tex);
    return max(f32(dims.y), 2.0);
}

fn lut_cube_size_3d(requested: f32) -> f32 {
    if requested >= 2.0 {
        return requested;
    }
    let dims = textureDimensions(lut_3d_tex);
    return max(f32(dims.x), 2.0);
}

fn lut_2d_sample(rgb: vec3<f32>, lut_size: f32) -> vec3<f32> {
    let size = max(lut_size, 2.0);
    let dims_u = textureDimensions(lut_2d_tex);
    let dims = vec2<f32>(f32(dims_u.x), f32(dims_u.y));
    let clamped = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let blue = clamped.b * (size - 1.0);
    let slice0 = floor(blue);
    let slice1 = min(slice0 + 1.0, size - 1.0);
    let lerp_b = blue - slice0;
    let x0 = (clamped.r * (size - 1.0) + 0.5 + slice0 * size) / dims.x;
    let x1 = (clamped.r * (size - 1.0) + 0.5 + slice1 * size) / dims.x;
    let y = (clamped.g * (size - 1.0) + 0.5) / dims.y;
    let c0 = textureSample(lut_2d_tex, input_sampler, vec2<f32>(x0, y)).rgb;
    let c1 = textureSample(lut_2d_tex, input_sampler, vec2<f32>(x1, y)).rgb;
    return mix(c0, c1, lerp_b);
}

fn lut_2d_apply(color: vec4<f32>, strength: f32, requested_size: f32) -> vec4<f32> {
    let s = clamp(strength, 0.0, 1.0);
    let lut_size = lut_cube_size_2d(requested_size);
    let graded = lut_2d_sample(color.rgb, lut_size);
    return vec4<f32>(mix(color.rgb, graded, s), color.a);
}

fn lut_3d_apply(color: vec4<f32>, strength: f32, requested_size: f32) -> vec4<f32> {
    let s = clamp(strength, 0.0, 1.0);
    let lut_size = lut_cube_size_3d(requested_size);
    let half_texel = 0.5 / lut_size;
    let coord = mix(
        vec3<f32>(half_texel),
        vec3<f32>(1.0 - half_texel),
        clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)),
    );
    let graded = textureSample(lut_3d_tex, input_sampler, coord).rgb;
    return vec4<f32>(mix(color.rgb, graded, s), color.a);
}
