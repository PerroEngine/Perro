fn reverse_filter_apply(
    color: vec4<f32>,
    target_color: vec3<f32>,
    strength: f32,
    softness: f32,
) -> vec4<f32> {
    let s = clamp(strength, 0.0, 1.0);
    let soft = max(softness, 0.0001);
    let target_norm = normalize(max(target_color, vec3<f32>(0.0001)));
    let c_norm = normalize(max(color.rgb, vec3<f32>(0.0001)));
    let match_factor = clamp(dot(c_norm, target_norm), 0.0, 1.0);
    let keep = smoothstep(1.0 - soft, 1.0, match_factor);
    let luma = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let washed = mix(vec3<f32>(luma), color.rgb, keep);
    let out_rgb = mix(color.rgb, washed, s);
    return vec4<f32>(out_rgb, color.a);
}
