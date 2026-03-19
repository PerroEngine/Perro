fn color_filter_apply(color: vec4<f32>, tint: vec3<f32>, strength: f32) -> vec4<f32> {
    let s = clamp(strength, 0.0, 1.0);
    let filtered = color.rgb * tint;
    let out_rgb = mix(color.rgb, filtered, s);
    return vec4<f32>(out_rgb, color.a);
}
