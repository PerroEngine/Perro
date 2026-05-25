fn sky_shader(in: SkyFragment) -> vec4<f32> {
    let width = max(custom_param(in, 0u).x, 0.001);
    let strength = clamp(custom_param(in, 1u).x, 0.0, 1.0);
    let band = (1.0 - smoothstep(0.0, width, abs(in.ray.y))) * strength;
    return vec4<f32>(mix(in.color.rgb, custom_param(in, 2u).rgb, band), in.color.a);
}
