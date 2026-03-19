fn vignette_apply(
    uv: vec2<f32>,
    color: vec4<f32>,
    strength: f32,
    radius: f32,
    softness: f32,
) -> vec4<f32> {
    let s = clamp(strength, 0.0, 1.0);
    if s <= 0.001 {
        return color;
    }
    let r = clamp(radius, 0.0, 1.0);
    let soft = max(softness, 0.0001);
    let dist = distance(uv, vec2<f32>(0.5, 0.5));
    let edge0 = r;
    let edge1 = r + soft;
    let t = smoothstep(edge0, edge1, dist);
    let factor = mix(1.0, 1.0 - s, t);
    return vec4<f32>(color.rgb * factor, color.a);
}
