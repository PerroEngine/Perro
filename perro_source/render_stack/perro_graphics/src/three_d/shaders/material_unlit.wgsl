fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let emissive = unpack_rgba8(in.packed_emissive).xyz;
    let material = decode_material_params(in.packed_material_params);
    var n = normalize(in.normal_ws);
    if material.flat_shading {
        n = normalize(cross(dpdx(in.world_pos), dpdy(in.world_pos)));
    }
    if material.double_sided && !in.is_front {
        n = -n;
    }
    var alpha = clamp(color.a, 0.0, 1.0);
    if material.alpha_mode == 1u && alpha < material.alpha_cutoff {
        discard;
    }
    if material.alpha_mode == 0u {
        alpha = 1.0;
    }
    if material.meshlet_debug_view {
        return vec4<f32>(color.rgb, 1.0);
    }

    let shaded = color.rgb + emissive;
    return vec4<f32>(shaded, alpha);
}
