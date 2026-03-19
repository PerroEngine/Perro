fn shade_material(in: FragmentInput) -> vec4<f32> {
    let alpha_mode = u32(in.material_params.x + 0.5);
    let alpha_cutoff = clamp(in.material_params.y, 0.0, 1.0);
    var alpha = clamp(in.color.a, 0.0, 1.0);
    if alpha_mode == 1u && alpha < alpha_cutoff {
        discard;
    }
    if alpha_mode == 0u {
        alpha = 1.0;
    }

    let p = in.world_pos * vec3<f32>(0.35, 0.55, 0.85);
    let stripe = sin(p.x * 6.0) * 0.5 + 0.5;
    let wave = sin(p.y * 4.0 + p.x * 2.0) * 0.5 + 0.5;
    let swirl = sin(p.z * 5.0 + p.y * 3.0) * 0.5 + 0.5;

    let color = vec3<f32>(
        0.2 + 0.8 * stripe,
        0.2 + 0.8 * wave,
        0.2 + 0.8 * swirl
    );
    return vec4<f32>(color, alpha);
}
