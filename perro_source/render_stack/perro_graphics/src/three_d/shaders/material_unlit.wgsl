fn shade_material(in: FragmentInput) -> vec4<f32> {
    let double_sided = in.material_params.z > 0.5;
    var n = normalize(in.normal_ws);
    if double_sided && !in.is_front {
        n = -n;
    }
    let alpha_mode = u32(in.material_params.x + 0.5);
    let alpha_cutoff = clamp(in.material_params.y, 0.0, 1.0);
    let meshlet_debug_view = in.material_params.w > 0.5;
    var alpha = clamp(in.color.a, 0.0, 1.0);
    if alpha_mode == 1u && alpha < alpha_cutoff {
        discard;
    }
    if alpha_mode == 0u {
        alpha = 1.0;
    }
    if meshlet_debug_view {
        return vec4<f32>(in.color.rgb, 1.0);
    }

    let color = in.color.rgb + in.emissive_factor;
    return vec4<f32>(color, alpha);
}
