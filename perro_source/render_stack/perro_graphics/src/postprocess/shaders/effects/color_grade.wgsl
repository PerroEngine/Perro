fn color_grade_hue_shift(rgb: vec3<f32>, hue_shift: f32) -> vec3<f32> {
    let angle = hue_shift * 6.28318530718;
    let s = sin(angle);
    let c = cos(angle);
    let yiq = mat3x3<f32>(
        vec3<f32>(0.299, 0.596, 0.211),
        vec3<f32>(0.587, -0.274, -0.523),
        vec3<f32>(0.114, -0.322, 0.312),
    ) * rgb;
    let shifted = vec3<f32>(
        yiq.x,
        yiq.y * c - yiq.z * s,
        yiq.y * s + yiq.z * c,
    );
    return mat3x3<f32>(
        vec3<f32>(1.0, 1.0, 1.0),
        vec3<f32>(0.956, -0.272, -1.106),
        vec3<f32>(0.621, -0.647, 1.703),
    ) * shifted;
}

fn color_grade_apply(color: vec4<f32>) -> vec4<f32> {
    return color_grade_apply_params(
        color,
        post.params0,
        post.params1,
        post.params2,
        post.params3,
        post.params4,
    );
}

fn color_grade_apply_params(
    color: vec4<f32>,
    p0: vec4<f32>,
    p1: vec4<f32>,
    p2: vec4<f32>,
    p3: vec4<f32>,
    p4: vec4<f32>,
) -> vec4<f32> {
    let exposure = p0.x;
    let contrast = max(p0.y, 0.0);
    let brightness = p0.z;
    let saturation = max(p0.w, 0.0);
    let gamma_value = max(p1.x, 0.001);
    let temperature = p1.y;
    let tint = p1.z;
    let hue_shift = p1.w;
    let lift = p2.xyz;
    let vibrance = p2.w;
    let gain = p3.xyz;
    let offset = p4.xyz;

    var rgb = max(color.rgb, vec3<f32>(0.0));
    rgb *= exp2(exposure);
    rgb += vec3<f32>(temperature, tint, -temperature) * 0.1;
    rgb = color_grade_hue_shift(rgb, hue_shift);
    rgb = (rgb - vec3<f32>(0.5)) * contrast + vec3<f32>(0.5 + brightness);

    let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    rgb = mix(vec3<f32>(luma), rgb, saturation);
    let high = max(max(rgb.r, rgb.g), rgb.b);
    let low = min(min(rgb.r, rgb.g), rgb.b);
    let chroma = clamp(high - low, 0.0, 1.0);
    rgb = mix(vec3<f32>(luma), rgb, 1.0 + vibrance * (1.0 - chroma));

    rgb = (rgb + lift) * gain + offset;
    rgb = pow(max(rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / gamma_value));
    return vec4<f32>(rgb, color.a);
}
