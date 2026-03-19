fn saturate_apply(color: vec4<f32>, amount: f32) -> vec4<f32> {
    let a = max(amount, 0.0);
    let luma = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let out_rgb = mix(vec3<f32>(luma), color.rgb, a);
    return vec4<f32>(out_rgb, color.a);
}
