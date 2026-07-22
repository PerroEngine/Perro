fn chroma_key_apply(
    color: vec4<f32>,
    key_color: vec3<f32>,
    tolerance: f32,
    softness: f32,
) -> vec4<f32> {
    let distance = length(color.rgb - key_color) / sqrt(3.0);
    let edge0 = max(tolerance, 0.0);
    let edge1 = edge0 + max(softness, 0.000001);
    let keep = smoothstep(edge0, edge1, distance);
    return vec4<f32>(color.rgb, color.a * keep);
}
